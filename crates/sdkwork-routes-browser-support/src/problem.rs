use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sdkwork_browser_platform_service::PlatformError;
use sdkwork_utils_rust::SdkWorkResultCode;
use sdkwork_web_core::{
    problem_response, ProblemCorrelation, WebFrameworkError, WebFrameworkErrorKind,
};

pub type BrowserApiResult<T> = Result<T, BrowserApiError>;

#[derive(Debug, Clone)]
pub struct BrowserApiError {
    status: StatusCode,
    detail: String,
}

impl BrowserApiError {
    pub fn new(status: StatusCode, detail: impl Into<String>) -> Self {
        Self {
            status,
            detail: detail.into(),
        }
    }

    pub fn from_result_code(code: SdkWorkResultCode, detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::from_u16(code.http_status_code()).unwrap_or(StatusCode::BAD_REQUEST),
            detail,
        )
    }

    fn framework_error(&self) -> WebFrameworkError {
        let kind = match self.status {
            StatusCode::BAD_REQUEST => WebFrameworkErrorKind::BadRequest,
            StatusCode::UNAUTHORIZED => WebFrameworkErrorKind::MissingCredentials,
            StatusCode::FORBIDDEN => WebFrameworkErrorKind::Forbidden,
            StatusCode::NOT_FOUND => WebFrameworkErrorKind::NotFound,
            StatusCode::CONFLICT => WebFrameworkErrorKind::Conflict,
            StatusCode::NOT_IMPLEMENTED => WebFrameworkErrorKind::NotImplemented,
            _ if self.status.is_server_error() => WebFrameworkErrorKind::InternalServerError,
            _ => WebFrameworkErrorKind::BadRequest,
        };
        WebFrameworkError {
            kind,
            message: self.detail.clone(),
            retry_after_seconds: None,
            auth_profile: None,
            failed_stage: None,
            reason: None,
        }
    }
}

impl From<PlatformError> for BrowserApiError {
    fn from(error: PlatformError) -> Self {
        match error {
            PlatformError::InvalidEngine(value) => Self::from_result_code(
                SdkWorkResultCode::InvalidParameter,
                format!("invalid engine config: {value}"),
            ),
            PlatformError::EngineNotStarted => {
                Self::from_result_code(SdkWorkResultCode::Conflict, "engine not started")
            }
            PlatformError::Registry(error) => {
                Self::from_result_code(SdkWorkResultCode::NotFound, error.to_string())
            }
            PlatformError::Engine(error) => {
                Self::from_result_code(SdkWorkResultCode::InternalError, error.to_string())
            }
            // Startup-only configuration failure; never produced by a live
            // request path, and still mapped so a future caller cannot panic.
            PlatformError::LlmConfig(error) => {
                Self::from_result_code(SdkWorkResultCode::InternalError, error.to_string())
            }
        }
    }
}

impl From<SdkWorkResultCode> for BrowserApiError {
    fn from(code: SdkWorkResultCode) -> Self {
        Self::from_result_code(code, code.title())
    }
}

#[derive(Debug, Clone)]
pub struct BrowserApiProblem {
    error: BrowserApiError,
}

impl BrowserApiProblem {
    pub fn new(error: BrowserApiError) -> Self {
        Self { error }
    }
}

impl From<BrowserApiError> for BrowserApiProblem {
    fn from(error: BrowserApiError) -> Self {
        Self { error }
    }
}

impl From<PlatformError> for BrowserApiProblem {
    fn from(error: PlatformError) -> Self {
        BrowserApiError::from(error).into()
    }
}

impl From<SdkWorkResultCode> for BrowserApiProblem {
    fn from(code: SdkWorkResultCode) -> Self {
        BrowserApiError::from(code).into()
    }
}

impl IntoResponse for BrowserApiProblem {
    fn into_response(self) -> Response {
        problem_response(
            &self.error.framework_error(),
            ProblemCorrelation::new(None, None),
        )
    }
}
