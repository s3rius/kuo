use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::{api::ObjectMeta, CustomResourceExt, ResourceExt};

pub trait ObjectMetaKuoExt: Default {
    fn default_with_owner<T>(owner: &T) -> Self
    where
        T: CustomResourceExt + ResourceExt;
}

impl ObjectMetaKuoExt for ObjectMeta {
    fn default_with_owner<T>(owner: &T) -> Self
    where
        T: CustomResourceExt + ResourceExt,
    {
        let mut meta = ObjectMeta::default();
        let mut labels = std::collections::BTreeMap::new();
        labels.insert(
            String::from("app.kubernetes.io/managed-by"),
            String::from("kuo-operator"),
        );
        let api_resounce = T::api_resource();
        meta.owner_references = Some(vec![OwnerReference {
            api_version: api_resounce.api_version,
            kind: api_resounce.kind,
            name: String::from(owner.name_any()),
            uid: String::from(owner.meta().uid.as_ref().unwrap()),
            controller: Some(true),
            block_owner_deletion: Some(false),
        }]);
        meta.labels = Some(labels);
        meta
    }
}
