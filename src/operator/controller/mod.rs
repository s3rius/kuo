use std::{sync::Arc, time::Duration};

use futures::StreamExt;
use k8s_openapi::api::certificates::v1::CertificateSigningRequest;
use kube::{runtime::controller::Action, Api};

use crate::{crds::managed_user::ManagedUser, operator::error::KuoError};

use super::ctx::OperatorCtx;

pub mod csr;
mod managed_user;

pub fn default_on_error<T>(_: Arc<T>, error: &KuoError, _ctx: Arc<OperatorCtx>) -> Action
where
    T: Clone
        + kube::Resource<DynamicType = ()>
        + serde::de::DeserializeOwned
        + std::fmt::Debug
        + Send
        + Sync
        + 'static,
    T::DynamicType: Eq + std::hash::Hash + Clone,
{
    match error {
        KuoError::CannotReconcile(_) => Action::requeue(Duration::from_secs(60 * 5)),
        _ => Action::requeue(Duration::from_secs(60)),
    }
}

pub async fn run(ctx: Arc<OperatorCtx>) -> anyhow::Result<()> {
    tracing::info!("Running operator controller");
    let managed_user_controller = kube::runtime::Controller::new(
        Api::<ManagedUser>::all(ctx.client.clone()),
        kube::runtime::watcher::Config::default(),
    )
    .run(
        managed_user::reconcile,
        default_on_error::<ManagedUser>,
        ctx.clone(),
    )
    .for_each(|_| futures::future::ready(()));
    let csr_controller = kube::runtime::Controller::new(
        Api::<CertificateSigningRequest>::all(ctx.client.clone()),
        kube::runtime::watcher::Config {
            label_selector: Some(String::from("app.kubernetes.io/managed-by=kuo-operator")),
            ..Default::default()
        },
    )
    .run(
        csr::reconcile,
        default_on_error::<CertificateSigningRequest>,
        ctx.clone(),
    )
    .for_each(|_| futures::future::ready(()));

    tokio::select! {
        _ = managed_user_controller => {
            tracing::warn!("Managed user controller stopped. Exiting.");
        }
        _ = csr_controller => {
            tracing::warn!("CSR controller stopped. Exiting.");
        }
    }
    Ok(())
}
