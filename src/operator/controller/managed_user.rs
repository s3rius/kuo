use std::{sync::Arc, time::Duration};

use k8s_openapi::{api::certificates::v1::CertificateSigningRequest, ByteString};
use kube::{
    api::{ObjectMeta, PostParams},
    runtime::{controller::Action, reflector::Lookup},
    ResourceExt,
};

use crate::{
    crds::managed_user::{ManagedUser, ManagedUserStatus},
    operator::{
        ctx::OperatorCtx,
        error::{KuoError, KuoResult},
        utils::{meta::ObjectMetaKuoExt, resource::KuoResourceExt},
    },
};

fn gen_user_pkey() -> KuoResult<openssl::pkey::PKey<openssl::pkey::Private>> {
    tracing::info!("Generating RSA key");
    let rsa = openssl::rsa::Rsa::generate(4096)?;
    Ok(openssl::pkey::PKey::from_rsa(rsa)?)
}

fn build_csr(
    username: &str,
    pkey: &openssl::pkey::PKey<openssl::pkey::Private>,
) -> KuoResult<openssl::x509::X509Req> {
    tracing::info!("Building Certificate Signing Request (CSR)");
    let mut req_builder = openssl::x509::X509Req::builder()?;
    let mut x509_name = openssl::x509::X509NameBuilder::new()?;
    x509_name.append_entry_by_text("CN", username)?;
    req_builder.set_subject_name(&x509_name.build())?;
    req_builder.set_pubkey(&pkey)?;
    req_builder.sign(&pkey, openssl::hash::MessageDigest::sha256())?;
    tracing::info!("CSR built successfully");
    Ok(req_builder.build())
}

pub async fn create_kube_csr(
    ctx: Arc<OperatorCtx>,
    user: &ManagedUser,
    x509_req: &openssl::x509::X509Req,
    csr_name: &str,
) -> KuoResult<CertificateSigningRequest> {
    let cert_req_api = kube::Api::<CertificateSigningRequest>::all(ctx.client.clone());
    let mut meta = ObjectMeta::default();
    meta.insert_label("app.kubernetes.io/managed-by", "kuo-operator");
    meta.name = Some(csr_name.to_string());
    meta.add_owner(user, None);
    let sign_req = cert_req_api
        .create(
            &PostParams::default(),
            &CertificateSigningRequest {
                metadata: meta,
                spec: k8s_openapi::api::certificates::v1::CertificateSigningRequestSpec {
                    request: ByteString(x509_req.to_pem()?),
                    signer_name: ctx.args.signer_name.clone(),
                    usages: Some(vec![
                        String::from("digital signature"),
                        String::from("key encipherment"),
                        String::from("client auth"),
                    ]),
                    ..Default::default()
                },
                status: None,
            },
        )
        .await?;
    Ok(sign_req)
}

#[tracing::instrument(skip(user, ctx), fields(email = user.spec.email), err)]
pub async fn reconcile(user: Arc<ManagedUser>, ctx: Arc<OperatorCtx>) -> KuoResult<Action> {
    if user.name().is_none() {
        tracing::warn!("Managed user metadata has no name");
        return Err(KuoError::CannotReconcile(String::from(
            "Managed user metadata has no name",
        )));
    };
    if user.metadata.uid.is_none() {
        tracing::warn!("Managed user metadata has no UID");
        return Err(KuoError::CannotReconcile(String::from(
            "Managed user metadata has no UID",
        )));
    }
    user.sync_permissions(ctx.clone()).await?;
    if user.status.is_some() {
        return Ok(Action::requeue(Duration::from_secs(60 * 10)));
    }
    let pkey = gen_user_pkey()?;
    let username = user.name_unchecked();
    let csr = build_csr(&username, &pkey)?;
    let csr_name = format!("kuo-{}", &username);
    user.simple_patch_status(
        kube::Api::all(ctx.client.clone()),
        ManagedUserStatus {
            pkey: String::from_utf8(pkey.private_key_to_pem_pkcs8()?).unwrap(),
            cert: None,
        },
    )
    .await?;
    create_kube_csr(ctx, &user, &csr, &csr_name).await?;
    Ok(Action::requeue(Duration::from_secs(60 * 5)))
}
