use crate::operator::error::KuoResult;
use kube::ResourceExt;
use serde::{de::DeserializeOwned, Serialize};

pub(crate) trait KuoResourceExt<T>: kube::Resource + Sized {
    async fn patch_or_create(&self, api: kube::Api<T>) -> KuoResult<Self>;
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
}
