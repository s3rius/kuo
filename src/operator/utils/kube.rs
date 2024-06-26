use std::sync::Arc;

use k8s_openapi::api::core::v1::ConfigMap;

use crate::operator::{
    ctx::OperatorCtx,
    error::{KuoError, KuoResult},
};

pub async fn get_kube_cert(ctx: Arc<OperatorCtx>) -> KuoResult<String> {
    tracing::info!("Getting the kube root certificate.");
    let cm_name = &ctx.args.default_cert_name;
    let cmap =
        kube::Api::<ConfigMap>::namespaced(ctx.client.clone(), ctx.client.default_namespace())
            .get_opt(cm_name)
            .await?;
    let Some(cert_config_map) = cmap else {
        return Err(KuoError::CannotGetRootCert(format!(
            "The ConfigMap {cm_name} doesn't exist.",
        )));
    };
    let Some(cert_cm_data) = &cert_config_map.data else {
        return Err(KuoError::CannotGetRootCert(format!(
            "The ConfigMap {cm_name} has no data.",
        )));
    };
    let key = &ctx.args.default_cert_key;
    let Some(kube_cert) = cert_cm_data.get(key) else {
        return Err(KuoError::CannotGetRootCert(format!(
            "The key {key} doesn't exit in the ConfigMap."
        )));
    };
    Ok(kube_cert.clone())
}
