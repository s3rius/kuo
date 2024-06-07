use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::{api::ObjectMeta, ResourceExt};

pub trait ObjectMetaKuoExt: Default {
    fn add_owner<T>(&mut self, owner: &T, controller: Option<bool>)
    where
        T: kube::Resource<DynamicType = ()>,
        T::DynamicType: Eq + std::hash::Hash + Clone;

    fn insert_label<F, S>(&mut self, key: F, value: S)
    where
        F: ToString,
        S: ToString;
}

impl ObjectMetaKuoExt for ObjectMeta {
    fn add_owner<T>(&mut self, owner: &T, controller: Option<bool>)
    where
        T: kube::Resource<DynamicType = ()>,
        T::DynamicType: Eq + std::hash::Hash + Clone,
    {
        let mut owners = self.owner_references.take().unwrap_or_default();
        let owner = OwnerReference {
            api_version: String::from(T::api_version(&())),
            kind: String::from(T::kind(&())),
            name: String::from(owner.name_any()),
            uid: String::from(owner.meta().uid.as_ref().unwrap()),
            controller,
            block_owner_deletion: Some(false),
        };
        owners.push(owner);
        self.owner_references = Some(owners);
    }

    fn insert_label<F, S>(&mut self, key: F, value: S)
    where
        F: ToString,
        S: ToString,
    {
        let mut labels = self.labels.take().unwrap_or_default();
        labels.insert(key.to_string(), value.to_string());
        self.labels = Some(labels);
    }
}
