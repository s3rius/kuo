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
        utils::{get_kube_cert, resource::KuoResourceExt},
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
    let mut user = if let Some(owner) = owners.first() {
        let user = kube::Api::<ManagedUser>::all(ctx.client.clone())
            .get(&owner.name)
            .await?;
        if user.status.is_none() {
            return Err(KuoError::CannotReconcile(String::from(
                "User doesn't have a secret key.",
            )));
        }
        user
    } else {
        tracing::warn!("No owner found for CSR");
        return Ok(Action::requeue(Duration::from_secs(60 * 5)));
    };
    if let Some(CertificateSigningRequestStatus {
        certificate: Some(csr_signed_cert),
        conditions: _,
    }) = &csr_arc.status
    {
        // If the CSR has been signed and the user has a kubeconfig, we don't need to do anything.
        match &user.status {
            Some(ManagedUserStatus {
                cert: None,
                kubeconfig: None,
                pkey: private_key,
            }) => {
                tracing::info!("CSR has been signed. Generating kubeconfig.");
                let root_kube_cert = get_kube_cert(ctx.clone()).await?;
                let user_cert = String::from_utf8(csr_signed_cert.0.clone())?;
                let kubeconfig = serde_yaml::to_string(&user.build_kubeconfig(
                    &ctx.args.kube_addr,
                    private_key,
                    &user_cert,
                    &root_kube_cert,
                ))?;
                let mut new_status = user.status().cloned().unwrap();
                new_status.kubeconfig = Some(kubeconfig.clone());
                new_status.cert = Some(user_cert);
                user = user
                    .simple_patch_status(kube::Api::all(ctx.client.clone()), &new_status)
                    .await?;
                user.send_kubeconfig(ctx.clone(), &kubeconfig).await?;
                delete_csr(ctx.clone(), csr_arc.name_any().as_str()).await?;
                return Ok(Action::requeue(Duration::from_secs(60 * 10)));
            }
            Some(ManagedUserStatus {
                cert: Some(_),
                kubeconfig: Some(_),
                pkey: _,
            }) => {
                return Ok(Action::requeue(Duration::from_secs(60 * 10)));
            }
            _ => {
                tracing::warn!("User doesn't have a status");
                return Ok(Action::requeue(Duration::from_secs(60 * 5)));
            }
        }
    }
    let mut csr = Arc::unwrap_or_clone(csr_arc);
    approve_csr(&mut csr, ctx.clone()).await?;
    Ok(Action::requeue(Duration::from_secs(60 * 10)))
}
