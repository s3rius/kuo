use std::{
    collections::HashSet,
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};

use k8s_openapi::api::rbac::v1::{PolicyRule, Role, RoleBinding};
use kube::{
    api::{ObjectMeta, PostParams},
    ResourceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::operator::{ctx::OperatorCtx, error::KuoResult, utils::meta::ObjectMetaKuoExt};

use super::managed_user::ManagedUser;

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    /// APIGroups is the name of the APIGroup that contains the resources.
    /// If multiple API groups are specified,
    /// any action requested against one of the enumerated resources in any
    /// API group will be allowed.
    /// "" represents the core API group and "*" represents all API groups.
    pub api_groups: Option<Vec<String>>,
    /// Resources is a list of resources this rule applies to. '*' represents all resources.
    pub resources: Option<Vec<String>>,
    /// ResourceNames is an optional white list of names that the rule applies to.
    /// An empty set means that everything is allowed.
    pub resource_names: Option<Vec<String>>,
    /// NonResourceURLs is a set of partial urls that a user should have access to.  *s are allowed, but only as the full, final step in the path Since non-resource URLs are not namespaced, this field is only applicable for ClusterRoles referenced from a ClusterRoleBinding. Rules can either apply to API resources (such as "pods" or "secrets") or non-resource URL paths (such as "/api"),  but not both.
    pub non_resource_urls: Option<Vec<String>>,
    /// Verbs is a list of Verbs that apply to ALL the ResourceKinds contained in this rule. '*' represents all verbs.
    pub verbs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
pub struct NamespacedPermissions {
    // Name of the namespace to apply permission to.
    pub namespace: String,
    // List of permissions to apply to the namespace.
    pub permissions: Vec<Permission>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InlinePermissions {
    /// List of cluster-wide permissions.
    pub cluster_permissions: Option<Vec<Permission>>,
    /// List of namespaced permissions.
    pub namespaced_permissions: Option<Vec<NamespacedPermissions>>,
}

impl From<Permission> for PolicyRule {
    fn from(p: Permission) -> Self {
        PolicyRule {
            api_groups: p.api_groups,
            resources: p.resources,
            resource_names: p.resource_names,
            non_resource_urls: p.non_resource_urls,
            verbs: p.verbs,
        }
    }
}

impl NamespacedPermissions {
    pub async fn apply(&self, user: &ManagedUser, ctx: Arc<OperatorCtx>) -> KuoResult<String> {
        let mut hasher = DefaultHasher::new();
        let contents = serde_yaml::to_string(self)?;
        contents.hash(&mut hasher);
        let name = format!("{}-{}", user.name_any(), hasher.finish());
        let api = kube::Api::<Role>::namespaced(ctx.client.clone(), &self.namespace);
        let mut role_metadata = ObjectMeta::default_with_owner(user);
        role_metadata.name = Some(name.clone());
        api.create(
            &PostParams::default(),
            &Role {
                metadata: role_metadata,
                rules: Some(
                    self.permissions
                        .iter()
                        .cloned()
                        .map(PolicyRule::from)
                        .collect(),
                ),
            },
        )
        .await?;
        let api = kube::Api::<RoleBinding>::namespaced(ctx.client.clone(), &self.namespace);
        let mut rb_metadata = ObjectMeta::default_with_owner(user);
        rb_metadata.name = Some(name.clone());
        api.create(
            &PostParams::default(),
            &RoleBinding {
                metadata: rb_metadata,
                role_ref: k8s_openapi::api::rbac::v1::RoleRef {
                    api_group: String::from("rbac.authorization.k8s.io"),
                    kind: "Role".to_string(),
                    name: name.clone(),
                },
                subjects: Some(vec![k8s_openapi::api::rbac::v1::Subject {
                    kind: "User".to_string(),
                    name: user.name_any(),
                    namespace: None,
                    api_group: None,
                }]),
            },
        )
        .await?;
        Ok(name)
    }
}

impl InlinePermissions {
    pub async fn apply(&self, user: &ManagedUser, ctx: Arc<OperatorCtx>) -> KuoResult<()> {
        let mut known_namespaced_permissions = HashSet::new();
        if let Some(namespaced_permissions) = &self.namespaced_permissions {
            for namespaced in namespaced_permissions {
                let res = namespaced.apply(user, ctx.clone()).await;
                match res {
                    Ok(name) => {
                        known_namespaced_permissions.insert(name);
                    }
                    Err(err) => {
                        tracing::warn!("Failed to create namespaced permission. {err}");
                    }
                }
            }
        }
        Ok(())
    }
}
