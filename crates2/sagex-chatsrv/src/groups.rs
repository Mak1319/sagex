//! Group routes (the core of the service). All membership is tracked by
//! user object keys; usernames are resolved to keys at the boundary.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use mongodb::bson::doc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    auth::{ApiResult, api_err},
    models::Group,
    state::{AppState, AuthUser},
};

#[derive(Serialize)]
struct GroupOut {
    object_key: String,
    name: String,
    members: Vec<MemberOut>,
    admins: Vec<String>,
}

#[derive(Serialize)]
struct MemberOut {
    object_key: String,
    username: String,
}

async fn username_of(st: &AppState, key: &str) -> String {
    st.db
        .users
        .find_one(doc! { "_id": key })
        .await
        .ok()
        .flatten()
        .map(|u| u.username)
        .unwrap_or_else(|| key.to_string())
}

async fn to_out(st: &AppState, g: &Group) -> GroupOut {
    let mut members = Vec::with_capacity(g.members.len());
    for m in &g.members {
        members.push(MemberOut {
            object_key: m.clone(),
            username: username_of(st, m).await,
        });
    }
    GroupOut {
        object_key: g.object_key.clone(),
        name: g.name.clone(),
        members,
        admins: g.admins.clone(),
    }
}

async fn load_group(st: &AppState, id: &str) -> ApiResult<Group> {
    st.db
        .groups
        .find_one(doc! { "_id": id })
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "group not found"))
}

#[derive(Deserialize)]
pub struct CreateGroupBody {
    pub name: String,
    #[serde(default)]
    pub members: Vec<String>,
}

/// Create a group. `members` are usernames; the caller is always added
/// as member + admin.
pub async fn create_group(
    State(st): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateGroupBody>,
) -> ApiResult<impl IntoResponse> {
    let name = body.name.trim().to_string();
    if name.is_empty() || name.len() > 128 {
        return Err(api_err(StatusCode::BAD_REQUEST, "bad group name"));
    }
    let mut keys = vec![auth.object_key.clone()];
    for username in &body.members {
        let u = st
            .db
            .users
            .find_one(doc! { "username": username.trim() })
            .await
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        match u {
            Some(u) if !keys.contains(&u.object_key) => keys.push(u.object_key),
            Some(_) => {}
            None => {
                return Err(api_err(
                    StatusCode::BAD_REQUEST,
                    format!("unknown user: {username}"),
                ));
            }
        }
    }
    let group = Group {
        object_key: Uuid::now_v7().to_string(),
        name,
        members: keys.clone(),
        admins: vec![auth.object_key.clone()],
        created_by: auth.object_key,
        created_ms: Utc::now().timestamp_millis(),
    };
    st.db
        .groups
        .insert_one(group.clone())
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::CREATED, Json(json!(to_out(&st, &group).await))))
}

/// List groups the caller belongs to.
pub async fn list_groups(
    State(st): State<AppState>,
    auth: AuthUser,
) -> ApiResult<impl IntoResponse> {
    let mut cursor = st
        .db
        .groups
        .find(doc! { "members": &auth.object_key })
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut out = Vec::new();
    while cursor
        .advance()
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        let g: Group = cursor
            .deserialize_current()
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        out.push(to_out(&st, &g).await);
    }
    Ok(Json(json!(out)))
}

pub async fn get_group(
    State(st): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let g = load_group(&st, &id).await?;
    if !g.members.contains(&auth.object_key) {
        return Err(api_err(StatusCode::FORBIDDEN, "not a member"));
    }
    Ok(Json(json!(to_out(&st, &g).await)))
}

#[derive(Deserialize)]
pub struct AddMembersBody {
    pub members: Vec<String>,
}

/// Add members by username. Admin only.
pub async fn add_members(
    State(st): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<AddMembersBody>,
) -> ApiResult<impl IntoResponse> {
    let mut g = load_group(&st, &id).await?;
    if !g.admins.contains(&auth.object_key) {
        return Err(api_err(StatusCode::FORBIDDEN, "admin only"));
    }
    for username in &body.members {
        let u = st
            .db
            .users
            .find_one(doc! { "username": username.trim() })
            .await
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        match u {
            Some(u) if !g.members.contains(&u.object_key) => g.members.push(u.object_key),
            Some(_) => {}
            None => {
                return Err(api_err(
                    StatusCode::BAD_REQUEST,
                    format!("unknown user: {username}"),
                ));
            }
        }
    }
    st.db
        .groups
        .update_one(
            doc! { "_id": &g.object_key },
            doc! { "$set": { "members": &g.members } },
        )
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!(to_out(&st, &g).await)))
}

/// Remove a member. Admins can remove anyone; members can remove
/// themselves (leave). Last admin cannot leave while members remain.
pub async fn remove_member(
    State(st): State<AppState>,
    auth: AuthUser,
    Path((id, user_key)): Path<(String, String)>,
) -> ApiResult<impl IntoResponse> {
    let mut g = load_group(&st, &id).await?;
    if !g.members.contains(&user_key) {
        return Err(api_err(StatusCode::NOT_FOUND, "not a member"));
    }
    let is_self = user_key == auth.object_key;
    if !is_self && !g.admins.contains(&auth.object_key) {
        return Err(api_err(StatusCode::FORBIDDEN, "admin only"));
    }
    if g.admins.contains(&user_key) && g.admins.len() == 1 && g.members.len() > 1 {
        return Err(api_err(
            StatusCode::BAD_REQUEST,
            "last admin cannot leave while members remain",
        ));
    }
    g.members.retain(|m| m != &user_key);
    g.admins.retain(|a| a != &user_key);
    if g.members.is_empty() {
        st.db
            .groups
            .delete_one(doc! { "_id": &g.object_key })
            .await
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Json(json!({ "deleted": g.object_key })));
    }
    st.db
        .groups
        .update_one(
            doc! { "_id": &g.object_key },
            doc! { "$set": { "members": &g.members, "admins": &g.admins } },
        )
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!(to_out(&st, &g).await)))
}
