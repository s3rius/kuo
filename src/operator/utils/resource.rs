use crate::operator::error::KuoResult;
use kube::ResourceExt;
use serde::{de::DeserializeOwned, Serialize};

pub(crate) trait KuoResourceExt<T>: kube::Resource + Sized {
    async fn patch_or_create(&self, api: kube::Api<T>) -> KuoResult<Self>;
    async fn simple_patch_status<P: serde::Serialize + std::fmt::Debug>(
        &self,
        api: kube::Api<T>,
        status: P,
    ) -> KuoResult<Self>;
}

impl<K: kube::Resource<DynamicType = ()>> KuoResourceExt<K> for K
where
    K: kube::Resource<DynamicType = ()> + DeserializeOwned + Serialize + Clone + std::fmt::Debug,
{
    async fn patch_or_create(&self, api: kube::Api<K>) -> KuoResult<Self> {
        let meta = api.get_metadata_opt(self.name_any().as_str()).await?;
        let new_obj = if meta.is_none() {
            api.create(&Default::default(), &self).await?
        } else {
            api.patch(
                self.name_any().as_str(),
                &Default::default(),
                &kube::api::Patch::Merge(self),
            )
            .await?
        };
        Ok(new_obj)
    }

    async fn simple_patch_status<P: serde::Serialize + std::fmt::Debug>(
        &self,
        api: kube::Api<K>,
        status: P,
    ) -> KuoResult<Self> {
        let updated_obj = api
            .patch_status(
                &self.name_any(),
                &kube::api::PatchParams::default(),
                &kube::api::Patch::Merge(serde_json::json!({"status": status})),
            )
            .await?;
        Ok(updated_obj)
    }
}
