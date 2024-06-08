use std::{sync::Arc, time::Duration};

use k8s_openapi::{
    api::certificates::v1::{
        CertificateSigningRequest, CertificateSigningRequestCondition,
        CertificateSigningRequestStatus,
    },
    apimachinery::pkg::apis::meta::v1::Time,
};
use kube::{
    api::{DeleteParams, PatchParams},
    core::object::HasStatus,
    runtime::{controller::Action, reflector::Lookup},
    ResourceExt,
};

use crate::{
    crds::managed_user::{ManagedUser, ManagedUserStatus},
    operator::{
        ctx::OperatorCtx,
        error::{KuoError, KuoResult},
        utils::resource::KuoResourceExt,
    },
};

pub async fn get_user(client: kube::Client, name: String) -> KuoResult<ManagedUser> {
    let api = kube::Api::<ManagedUser>::all(client);
    let user = api.get(&name).await?;
    Ok(user)
}

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
    } else {
        csr.status = Some(
            k8s_openapi::api::certificates::v1::CertificateSigningRequestStatus {
                conditions: Some(vec![approve_condition]),
                certificate: None,
            },
        );
    };
    Ok(())
}

async fn approve_csr(csr: &mut CertificateSigningRequest, ctx: Arc<OperatorCtx>) -> KuoResult<()> {
    tracing::info!("Approving CSR");
    add_condition_if_needed(csr)?;
    let api = kube::Api::<CertificateSigningRequest>::all(ctx.client.clone());
    csr.managed_fields_mut().clear();
    tracing::info!("Patching CSR");
    api.patch_approval(
        csr.name_unchecked().as_str(),
        &PatchParams::default(),
        &kube::api::Patch::Merge(&csr),
    )
    .await?;
    Ok(())
}

async fn delete_csr(ctx: Arc<OperatorCtx>, name: &str) -> KuoResult<()> {
    let api = kube::Api::<CertificateSigningRequest>::all(ctx.client.clone());
    api.delete(name, &DeleteParams::default()).await?;
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
    let mut user = match owners.get(0) {
        Some(owner) => {
            let user = get_user(ctx.client.clone(), owner.name.clone()).await?;
            if user.status.is_none() {
                return Err(KuoError::CannotReconcile(String::from(
                    "User doesn't have a secret key.",
                )));
            }
            user
        }
        None => {
            tracing::warn!("No owner found for CSR");
            return Ok(Action::requeue(Duration::from_secs(60 * 5)));
        }
    };
    match &csr_arc.status {
        Some(CertificateSigningRequestStatus {
            certificate: Some(csr_signed_cert),
            conditions: _,
        }) => {
            // If the CSR has been signed and the user has a certificate, we don't need to do anything.
            if let Some(ManagedUserStatus {
                cert: Some(_),
                pkey: _,
            }) = &user.status
            {
                return Ok(Action::requeue(Duration::from_secs(60 * 10)));
            }
            let mut new_status = user.status().cloned().unwrap();
            new_status.cert = Some(String::from_utf8(csr_signed_cert.0.clone())?);
            user = user
                .simple_patch_status(kube::Api::all(ctx.client.clone()), &new_status)
                .await?;
            user.send_kubeconfig(ctx.clone()).await?;
            delete_csr(ctx.clone(), csr_arc.name_any().as_str()).await?;
            return Ok(Action::requeue(Duration::from_secs(60 * 10)));
        }
        _ => {}
    }

    let mut csr = Arc::unwrap_or_clone(csr_arc);
    approve_csr(&mut csr, ctx.clone()).await?;
    Ok(Action::requeue(Duration::from_secs(60 * 10)))
}
