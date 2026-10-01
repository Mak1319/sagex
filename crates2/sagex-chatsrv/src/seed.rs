//! One-shot `--seed` loader for an `example-test-set` directory.
//! Explicit user action only — never runs on boot.

use std::path::Path;

use argon2::Argon2;
use password_hash::PasswordHasher;
use chrono::Utc;
use mongodb::bson::doc;
use serde::Deserialize;
use uuid::Uuid;

use crate::{db::Db, models::{Group, Message, User}};

#[derive(Deserialize)]
struct SeedUser {
    username: String,
    password: String,
    kem_pub: Option<String>,
    dsa_pub: Option<String>,
}

#[derive(Deserialize)]
struct SeedGroup {
    name: String,
    members: Vec<String>,
}

#[derive(Deserialize)]
struct SeedMessage {
    group: String,
    sender: String,
    body: String,
    content_type: Option<String>,
    rustfs_ref: Option<String>,
}

fn read_json<T: for<'de> Deserialize<'de>>(dir: &Path, name: &str) -> Result<Vec<T>, String> {
    let text = std::fs::read_to_string(dir.join(name)).map_err(|e| format!("{name}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{name}: {e}"))
}

pub async fn seed_from_dir(db: &Db, dir: &Path) -> Result<String, String> {
    let users: Vec<SeedUser> = read_json(dir, "users.json")?;
    let groups: Vec<SeedGroup> = read_json(dir, "groups.json")?;
    let messages: Vec<SeedMessage> = read_json(dir, "messages.json").unwrap_or_default();

    let mut n_users = 0;
    for u in &users {
        if db
            .users
            .find_one(doc! { "username": &u.username })
            .await
            .map_err(|e| e.to_string())?
            .is_some()
        {
            continue;
        }
        let pw_hash = Argon2::default()
            .hash_password(u.password.as_bytes())
            .map(|h| h.to_string())
            .map_err(|e| e.to_string())?;
        db.users
            .insert_one(
                User {
                    object_key: Uuid::now_v7().to_string(),
                    username: u.username.clone(),
                    pw_hash,
                    kem_pub: u.kem_pub.clone(),
                    dsa_pub: u.dsa_pub.clone(),
                    refresh_jti: None,
                    created_ms: Utc::now().timestamp_millis(),
                },
                
            )
            .await
            .map_err(|e| e.to_string())?;
        n_users += 1;
    }

    // username -> object_key for group/message wiring
    let mut keys = std::collections::HashMap::new();
    let mut cursor = db
        .users
        .find(doc! {})
        .await
        .map_err(|e| e.to_string())?;
    while cursor.advance().await.map_err(|e| e.to_string())? {
        let u: User = cursor.deserialize_current().map_err(|e| e.to_string())?;
        keys.insert(u.username.clone(), u.object_key.clone());
    }

    let mut n_groups = 0;
    for g in &groups {
        if db
            .groups
            .find_one(doc! { "name": &g.name })
            .await
            .map_err(|e| e.to_string())?
            .is_some()
        {
            continue;
        }
        let mut members = Vec::new();
        for m in &g.members {
            match keys.get(m) {
                Some(k) if !members.contains(k) => members.push(k.clone()),
                Some(_) => {}
                None => return Err(format!("seed group {}: unknown user {m}", g.name)),
            }
        }
        if members.is_empty() {
            return Err(format!("seed group {}: no members", g.name));
        }
        let creator = members[0].clone();
        db.groups
            .insert_one(
                Group {
                    object_key: Uuid::now_v7().to_string(),
                    name: g.name.clone(),
                    members: members.clone(),
                    admins: vec![creator.clone()],
                    created_by: creator,
                    created_ms: Utc::now().timestamp_millis(),
                },
                
            )
            .await
            .map_err(|e| e.to_string())?;
        n_groups += 1;
    }

    let mut group_ids = std::collections::HashMap::new();
    let mut cursor = db
        .groups
        .find(doc! {})
        .await
        .map_err(|e| e.to_string())?;
    while cursor.advance().await.map_err(|e| e.to_string())? {
        let g: Group = cursor.deserialize_current().map_err(|e| e.to_string())?;
        group_ids.insert(g.name.clone(), (g.object_key.clone(), g.members.clone()));
    }

    let mut n_msgs = 0;
    for m in &messages {
        let (gid, members) = group_ids
            .get(&m.group)
            .ok_or_else(|| format!("seed message: unknown group {}", m.group))?;
        let sender = keys
            .get(&m.sender)
            .ok_or_else(|| format!("seed message: unknown sender {}", m.sender))?;
        if !members.contains(sender) {
            return Err(format!("seed message: {} not in {}", m.sender, m.group));
        }
        db.messages
            .insert_one(
                Message {
                    object_key: Uuid::now_v7().to_string(),
                    group_id: gid.clone(),
                    sender_key: sender.clone(),
                    body: m.body.clone(),
                    content_type: m.content_type.clone(),
                    rustfs_ref: m.rustfs_ref.clone(),
                    created_ms: Utc::now().timestamp_millis(),
                },
                
            )
            .await
            .map_err(|e| e.to_string())?;
        n_msgs += 1;
    }

    Ok(format!("seeded {n_users} users, {n_groups} groups, {n_msgs} messages"))
}
