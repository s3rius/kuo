use std::{collections::BTreeMap, str::FromStr, sync::Arc};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use k8s_openapi::{api::core::v1::Secret, Metadata};
use kube::{api::ObjectMeta, config::NamedContext, CustomResource, ResourceExt};
use lettre::{
    message::{header::ContentType, Attachment, Mailbox, SinglePart},
    Address, AsyncTransport,
};
use schemars::{schema::SchemaObject, JsonSchema};
use serde::{Deserialize, Serialize};

use crate::operator::{
    ctx::OperatorCtx,
    error::{KuoError, KuoResult},
    utils::{meta::ObjectMetaKuoExt, resource::KuoResourceExt},
};

use super::inline_permissions::InlinePermissions;

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[kube(
    group = "kuo.github.io",
    version = "v1",
    kind = "ManagedUser",
    printcolumn = r#"
    {
        "name":"Email", 
        "type":"string", 
        "description":"User's email", 
        "jsonPath":".spec.email"
    }, 
    {
        "name": "Full Name", 
        "type": "string", 
        "description": "User's real name", 
        "jsonPath": ".spec.full_name"
    }
    "#
)]
#[serde(rename_all = "camelCase")]
pub struct ManagedUserCRD {
    /// Email to use for sending kubeconfig
    #[schemars(schema_with = "immutable_rule::<Option<String>>")]
    #[validate(email)]
    #[serde(default)]
    pub email: Option<String>,
    /// User's full name. Used in email.
    #[serde(default)]
    pub full_name: Option<String>,
    /// List of inlined permissions.
    #[serde(default)]
    pub inline_permissions: Option<InlinePermissions>,
}

/// Struct that holds user's secret data
/// used to access kubernetes.
#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub struct ManagedUserSecretData {
    /// This is a private key, used by the client.
    /// This key is used for issuing certificate sign requests.
    pub pkey: String,
    /// Resulting certificate for a user.
    pub cert: Option<String>,
    /// Generated Kubeconfig
    pub kubeconfig: Option<String>,
}

impl From<&ManagedUserSecretData> for std::collections::BTreeMap<String, k8s_openapi::ByteString> {
    fn from(value: &ManagedUserSecretData) -> Self {
        let mut map = BTreeMap::new();
        map.insert(
            "pkey".to_string(),
            k8s_openapi::ByteString(value.pkey.bytes().collect()),
        );
        if let Some(cert) = &value.cert {
            map.insert(
                "cert".to_string(),
                k8s_openapi::ByteString(cert.bytes().collect()),
            );
        }
        if let Some(kubeconfig) = &value.kubeconfig {
            map.insert(
                "kubeconfig".to_string(),
                k8s_openapi::ByteString(kubeconfig.bytes().collect()),
            );
        }
        map
    }
}

impl TryFrom<std::collections::BTreeMap<String, k8s_openapi::ByteString>>
    for ManagedUserSecretData
{
    type Error = KuoError;

    fn try_from(
        value: std::collections::BTreeMap<String, k8s_openapi::ByteString>,
    ) -> Result<Self, Self::Error> {
        let Some(pkey) = value.get("pkey") else {
            return Err(KuoError::InvalidUserSecretData);
        };
        let pkey = String::from_utf8(pkey.0.clone())?;
        let cert = value
            .get("cert")
            .and_then(|v| String::from_utf8(v.0.clone()).ok());
        let kubeconfig = value
            .get("kubeconfig")
            .and_then(|v| String::from_utf8(v.0.clone()).ok());
        Ok(ManagedUserSecretData {
            pkey,
            cert,
            kubeconfig,
        })
    }
}

/// Add immutable rule to the field.
///
/// This rule will prevent the field from being changed.
///
/// # Panics
///
/// This function will panic if the generated schema is incorrect.
pub fn immutable_rule<T: JsonSchema>(
    gen: &mut schemars::gen::SchemaGenerator,
) -> schemars::schema::Schema {
    let mut schema: SchemaObject = T::json_schema(gen).into();
    schema.extensions.insert(
        String::from("x-kubernetes-validations"),
        serde_json::json!([
            {
                "rule": "self == oldSelf",
                "message": "Cannot change field. The value is immutable."
            }
        ]),
    );
    schema.into()
}

impl ManagedUser {
    #[inline]
    #[must_use]
    pub fn build_kubeconfig(
        &self,
        kube_addr: &str,
        private_key: &str,
        client_cert: &str,
        root_cert: &str,
    ) -> kube::config::Kubeconfig {
        let mut kubeconfig = kube::config::Kubeconfig::default();
        kubeconfig.clusters.push(kube::config::NamedCluster {
            name: String::from("cluster"),
            cluster: Some(kube::config::Cluster {
                server: Some(String::from(kube_addr)),
                certificate_authority_data: Some(BASE64_STANDARD.encode(root_cert)),
                ..Default::default()
            }),
        });
        kubeconfig.auth_infos.push(kube::config::NamedAuthInfo {
            name: self.name_any(),
            auth_info: Some(kube::config::AuthInfo {
                client_certificate_data: Some(BASE64_STANDARD.encode(client_cert)),
                client_key_data: Some(BASE64_STANDARD.encode(private_key).into()),
                ..Default::default()
            }),
        });
        kubeconfig.contexts.push(NamedContext {
            name: String::from("default"),
            context: Some(kube::config::Context {
                cluster: String::from("cluster"),
                user: self.name_any(),
                ..Default::default()
            }),
        });
        kubeconfig.current_context = Some(String::from("default"));
        kubeconfig
    }

    pub async fn send_kubeconfig(&self, ctx: Arc<OperatorCtx>, kubeconfig: &str) -> KuoResult<()> {
        let Some(email) = &self.spec.email else {
            return Ok(());
        };
        let Some(smtp) = &ctx.smtp else {
            return Ok(());
        };
        let Some(smtp_args) = &ctx.args.smtp_args else {
            tracing::warn!("Cannot send kubeconfig. SMTP not configured.",);
            return Ok(());
        };
        let kube_config_attachement = Attachment::new(String::from("kubeconfig.yaml"))
            .body(String::from(kubeconfig), ContentType::TEXT_PLAIN);
        let msg = lettre::Message::builder()
            .from(Mailbox::new(
                Some(smtp_args.from_name.clone()),
                lettre::Address::from_str(&smtp_args.from_email)?,
            ))
            .to(Mailbox::new(
                self.spec.full_name.clone(),
                Address::from_str(email)?,
            ))
            .subject("You've beed added to the Kubernetes cluster!")
            .date_now()
            .multipart(lettre::message::MultiPart::mixed().singlepart(SinglePart::html(format!(
                "Hello, <b>{}</b>! You've been added to the Kubernetes cluster. Please download the kubeconfig.",
                self.name_any(),
            )
            )).singlepart(kube_config_attachement))?;
        smtp.send(msg).await?;
        Ok(())
    }

    pub async fn get_secret(
        &self,
        api: kube::Api<Secret>,
    ) -> KuoResult<Option<ManagedUserSecretData>> {
        let name = format!("{}-data", self.name_any());
        let Some(key) = api.get_opt(&name).await? else {
            return Ok(None);
        };
        if let Some(data) = key.data {
            Ok(Some(ManagedUserSecretData::try_from(data)?))
        } else {
            Ok(None)
        }
    }

    pub async fn set_secret(
        &self,
        api: kube::Api<Secret>,
        data: &ManagedUserSecretData,
    ) -> KuoResult<()> {
        let name = format!("{}-data", self.name_any());
        let mut metadata = ObjectMeta::default();
        metadata.name = Some(name);
        metadata.add_owner(self);
        let secret = Secret {
            data: Some(data.into()),
            metadata,
            ..Default::default()
        };
        secret.patch_or_create(api).await?;
        Ok(())
    }

    #[inline]
    pub async fn sync_permissions(&self, ctx: Arc<OperatorCtx>) -> KuoResult<()> {
        tracing::info!("Syncing permissions");
        if let Some(permissions) = &self.spec.inline_permissions {
            permissions.apply(self, ctx.clone()).await?;
        }
        Ok(())
    }
}
