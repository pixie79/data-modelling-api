//! OpenAPI specification definition.
//!
//! Aggregates all route handlers and schemas for OpenAPI documentation generation.

use utoipa::{Modify, OpenApi};
#[derive(OpenApi)]
#[openapi(
    paths(
        // Authentication
        crate::routes::auth::initiate_github_login,
        crate::routes::auth::initiate_desktop_github_login,
        crate::routes::auth::handle_github_callback,
        crate::routes::auth::poll_auth_status,
        crate::routes::auth::exchange_auth_code,
        crate::routes::auth::refresh_token,
        crate::routes::auth::get_auth_status,
        crate::routes::auth::select_email,
        crate::routes::auth::logout,
        // Workspace
        crate::routes::workspace::create_workspace,
        crate::routes::workspace::get_workspace_info,
        crate::routes::workspace::list_profiles,
        crate::routes::workspace::list_domains,
        crate::routes::workspace::create_domain,
        crate::routes::workspace::get_domain,
        crate::routes::workspace::update_domain,
        crate::routes::workspace::delete_domain,
        crate::routes::workspace::load_domain,
        // Tables
        crate::routes::workspace::get_domain_tables,
        crate::routes::workspace::create_domain_table,
        crate::routes::workspace::get_domain_table,
        crate::routes::workspace::update_domain_table,
        crate::routes::workspace::delete_domain_table,
        // Relationships
        crate::routes::workspace::get_domain_relationships,
        crate::routes::workspace::create_domain_relationship,
        crate::routes::workspace::get_domain_relationship,
        crate::routes::workspace::update_domain_relationship,
        crate::routes::workspace::delete_domain_relationship,
        // Cross-domain
        crate::routes::workspace::get_cross_domain_config,
        crate::routes::workspace::list_cross_domain_tables,
        crate::routes::workspace::add_cross_domain_table,
        crate::routes::workspace::update_cross_domain_table_ref,
        crate::routes::workspace::remove_cross_domain_table,
        crate::routes::workspace::list_cross_domain_relationships,
        crate::routes::workspace::remove_cross_domain_relationship,
        crate::routes::workspace::sync_cross_domain_relationships,
        // Canvas
        crate::routes::workspace::get_domain_canvas,
        // Import
        crate::routes::import::import_sql,
        crate::routes::import::import_sql_text,
        crate::routes::import::import_odcl,
        crate::routes::import::import_odcl_text,
        crate::routes::import::import_avro,
        crate::routes::import::import_json_schema,
        crate::routes::import::import_protobuf,
        // Export
        crate::routes::models::export_format,
        crate::routes::models::export_all,
        // DrawIO
        crate::routes::drawio::export_drawio,
        crate::routes::drawio::import_drawio,
        // Git Sync
        crate::routes::git_sync::get_sync_config,
        crate::routes::git_sync::update_sync_config,
        crate::routes::git_sync::init_repository,
        crate::routes::git_sync::clone_repository,
        crate::routes::git_sync::get_sync_status,
        crate::routes::git_sync::export_domain,
        crate::routes::git_sync::commit_changes,
        crate::routes::git_sync::push_changes,
        crate::routes::git_sync::pull_changes,
        crate::routes::git_sync::list_conflicts,
        crate::routes::git_sync::resolve_conflict,
        // Collaboration
        crate::routes::collaboration_sessions::create_session,
        crate::routes::collaboration_sessions::list_sessions,
        crate::routes::collaboration_sessions::get_session,
        crate::routes::collaboration_sessions::end_session,
        crate::routes::collaboration_sessions::list_participants,
        crate::routes::collaboration_sessions::invite_user,
        crate::routes::collaboration_sessions::remove_participant,
        crate::routes::collaboration_sessions::list_pending_requests,
        crate::routes::collaboration_sessions::request_access,
        crate::routes::collaboration_sessions::respond_to_request,
        crate::routes::collaboration_sessions::get_presence,
        // Audit
        crate::routes::audit::get_domain_history,
        crate::routes::audit::get_table_history,
        crate::routes::audit::get_relationship_history,
        crate::routes::audit::get_workspace_history,
        crate::routes::audit::get_audit_entry,
        // AI
        crate::routes::ai::resolve_errors,
        // OpenAPI
        crate::routes::openapi::serve_openapi_json,
    ),
    components(schemas(
        // Import types from models
        crate::models::Table,
        crate::models::Column,
        crate::models::Relationship,
        crate::models::DataModel,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "Authentication", description = "GitHub OAuth authentication endpoints"),
        (name = "Workspace", description = "Workspace and domain management"),
        (name = "Tables", description = "Table CRUD operations"),
        (name = "Relationships", description = "Relationship CRUD operations"),
        (name = "Import", description = "Multi-format import endpoints"),
        (name = "Export", description = "Multi-format export endpoints"),
        (name = "Git Sync", description = "Git synchronization operations"),
        (name = "Collaboration", description = "Real-time collaboration sessions"),
        (name = "Audit", description = "Audit trail queries"),
        (name = "AI", description = "AI-powered error resolution"),
        (name = "OpenAPI", description = "OpenAPI specification"),
    ),
    info(
        title = "Data Modelling API",
        description = "REST API for data modeling, schema management, and collaboration",
        version = "1.1.0",
        contact(
            name = "API Support",
            email = "mark@olliver.me.uk"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    servers(
        (url = "http://localhost:8081/api/v1", description = "Local development server"),
        (url = "https://api.example.com/api/v1", description = "Production server")
    )
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        // Update version to match Cargo.toml version
        openapi.info.version = env!("CARGO_PKG_VERSION").to_string();

        // Initialize components if they don't exist
        if openapi.components.is_none() {
            openapi.components = Some(utoipa::openapi::Components::new());
        }

        let components = openapi.components.as_mut().unwrap();
        use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityScheme};
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
        );
        components.add_security_scheme(
            "api_key",
            SecurityScheme::ApiKey(utoipa::openapi::security::ApiKey::Header(
                utoipa::openapi::security::ApiKeyValue::new("X-API-Key"),
            )),
        );
    }
}
