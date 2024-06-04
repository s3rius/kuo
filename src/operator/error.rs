pub type KuoResult<T> = Result<T, KuoError>;

#[derive(thiserror::Error, Debug)]
pub enum KuoError {
    #[error("Cannot reconcile: {0}")]
    CannotReconcile(String),
    #[error("CSR was denied")]
    CSRDenied,
    #[error("Cannot get root kube certificate. Reason: {0}")]
    CannotGetRootCert(String),
    #[error("Cannot generate kubeconfig. Reason: {0}")]
    CannotGenerateKubeconfig(String),
    #[error("StdError: {0}")]
    StdError(#[from] std::io::Error),
    #[error("OpensslError: {0}")]
    OpensslError(#[from] openssl::error::ErrorStack),
    #[error("KubeError: {0}")]
    KubeError(#[from] kube::Error),
    #[error("Utf8Error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
    #[error("Cannot send e-mail. Reason: {0}")]
    EmailSMTPError(#[from] lettre::transport::smtp::Error),
    #[error("Cannot send e-mail. Reason: {0}")]
    EmailAddressError(#[from] lettre::address::AddressError),
    #[error("Cannot send e-mail. Reason: {0}")]
    EmailError(#[from] lettre::error::Error),
    #[error("Cannot serialize/deserialize YAML. Reason: {0}")]
    YAMLError(#[from] serde_yaml::Error),
}
