use k8s_openapi::api::certificates::v1::CertificateSigningRequest;

pub mod permissions;
pub mod managed_user;

pub trait UniqueInfo {
    fn unique_info(&self) -> String;
}

impl UniqueInfo for CertificateSigningRequest {
    fn unique_info(&self) -> String {
        self.metadata
            .name
            .as_ref()
            .map_or_else(|| String::from("Unknown"), |name| name.clone())
    }
}
