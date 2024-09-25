use std::{sync::Arc, time::Duration};

use k8s_openapi::{
    api::{
        certificates::v1::{
            CertificateSigningRequest, CertificateSigningRequestCondition,
            CertificateSigningRequestStatus,
        },
        core::v1::Secret,
    },
    apimachinery::pkg::apis::meta::v1::Time,
};
use kube::{
    api::{DeleteParams, PatchParams},
    runtime::{controller::Action, reflector::Lookup},
    ResourceExt,
};

use crate::{
    crds::managed_user::ManagedUser,
    operator::{
        ctx::OperatorCtx,
        error::{KuoError, KuoResult},
        utils::get_kube_cert,
    },
};

fn add_condition_if_needed(csr: &mut CertificateSigningRequest) -> KuoResult<()> {
    let approve_condition = CertificateSigningRequestCondition {
        last_update_time: Some(Time(chrono::Utc::now())),
        message: Some(String::from("Certificate request approved by kuo")),
        reason: Some(String::from("Kuo generated request aprroved")),
        status: String::from("True"),
        type_: String::from("Approved"),
        last_transition_time: None,
    };
    if let Some(cert_status) = csr.status.as_mut() {
        if let Some(conditions) = cert_status.conditions.as_mut() {
            for condition in conditions.iter() {
                if condition.type_ == "Approved" {
                    return Ok(());
                } else if condition.type_ == "Denied" {
                    return Err(KuoError::CSRDenied);
                }
            }
            conditions.push(approve_condition);
            return Ok(());
        }
        cert_status.conditions = Some(vec![approve_condition]);
        return Ok(());
    }
    csr.status = Some(
        k8s_openapi::api::certificates::v1::CertificateSigningRequestStatus {
            conditions: Some(vec![approve_condition]),
            certificate: None,
        },
    );
    Ok(())
}

async fn approve_csr(csr: &mut CertificateSigningRequest, ctx: Arc<OperatorCtx>) -> KuoResult<()> {
    tracing::info!("Approving CSR");
    add_condition_if_needed(csr)?;
    csr.managed_fields_mut().clear();
    tracing::info!("Patching CSR");
    kube::Api::<CertificateSigningRequest>::all(ctx.client.clone())
        .patch_approval(
            csr.name_unchecked().as_str(),
            &PatchParams::default(),
            &kube::api::Patch::Merge(&csr),
        )
        .await?;
    Ok(())
}

async fn delete_csr(ctx: Arc<OperatorCtx>, name: &str) -> KuoResult<()> {
    kube::Api::<CertificateSigningRequest>::all(ctx.client.clone())
        .delete(name, &DeleteParams::default())
        .await?;
    Ok(())
}

#[tracing::instrument(skip(csr_arc, ctx), fields(name=csr_arc.name_any()), err)]
pub async fn reconcile(
    csr_arc: Arc<CertificateSigningRequest>,
    ctx: Arc<OperatorCtx>,
) -> KuoResult<Action> {
    tracing::info!("Reconciling CSR");
    if csr_arc.name().is_none() {
        tracing::warn!("CSR metadata has no name");
        return Err(KuoError::CannotReconcile(String::from(
            "CSR metadata has no name",
        )));
    };
    let owners = csr_arc.owner_references();
    let user = if let Some(owner) = owners.first() {
        kube::Api::<ManagedUser>::all(ctx.client.clone())
            .get(&owner.name)
            .await?
    } else {
        tracing::warn!("No owner found for CSR");
        return Ok(Action::requeue(Duration::from_secs(60 * 5)));
    };
    let Some(mut users_secret) = user
        .get_secret(kube::Api::<Secret>::namespaced(
            ctx.client.clone(),
            ctx.client.default_namespace(),
        ))
        .await?
    else {
        return Err(KuoError::CannotReconcile(String::from(
            "User doesn't have a secret key.",
        )));
    };
    if users_secret.cert.is_some() && users_secret.kubeconfig.is_some() {
        tracing::debug!("Certificate and kubeconfig exist for the user.");
        return Ok(Action::requeue(Duration::from_secs(60 * 10)));
    }
    if let Some(CertificateSigningRequestStatus {
        certificate: Some(csr_signed_cert),
        conditions: _,
    }) = &csr_arc.status
    {
        tracing::info!("CSR has been signed. Generating kubeconfig.");
        let root_kube_cert = get_kube_cert(ctx.clone()).await?;
        let user_cert = String::from_utf8(csr_signed_cert.0.clone())?;
        let kubeconfig = serde_yaml::to_string(&user.build_kubeconfig(
            &ctx.args.kube_addr,
            ctx.args.cluster_name.clone(),
            users_secret.pkey.as_str(),
            &user_cert,
            &root_kube_cert,
        ))?;
        users_secret.kubeconfig = Some(kubeconfig.clone());
        users_secret.cert = Some(user_cert);
        user.set_secret(
            kube::Api::namespaced(ctx.client.clone(), ctx.client.default_namespace()),
            &users_secret,
        )
        .await?;
        user.send_kubeconfig(ctx.clone(), &kubeconfig).await?;
        delete_csr(ctx.clone(), csr_arc.name_any().as_str()).await?;
        return Ok(Action::requeue(Duration::from_secs(60 * 10)));
    }
    let mut csr = Arc::unwrap_or_clone(csr_arc);
    approve_csr(&mut csr, ctx.clone()).await?;
    Ok(Action::requeue(Duration::from_secs(60 * 10)))
}
