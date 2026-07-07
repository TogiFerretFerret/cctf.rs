use super::*;

pub(crate) fn map_account(row: &sqlx::postgres::PgRow) -> Result<Account, sqlx::Error> {
    let id: String = row.get("id");
    let username: String = row.get("username");
    let email: Option<String> = row.get("email");
    let password_hash: Option<String> = row.get("password_hash");
    let role_str: String = row.get("role");
    let team_id_str: Option<String> = row.get("team_id");
    let ctftime_id: Option<i32> = row.get("ctftime_id");
    let fields_val: serde_json::Value = row.get("fields");
    let created_at: i64 = row.get("created_at");
    let role = match role_str.as_str() {
        "Admin" => AccountRole::Admin,
        "Spectator" => AccountRole::Spectator,
        _ => AccountRole::Player,
    };
    let fields =
        serde_json::from_value(fields_val).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    Ok(Account {
        id: AccountId(id),
        username: AccountName(username),
        email: email.map(AccountEmail),
        password_hash,
        role,
        team_id: team_id_str.map(TeamId),
        ctftime_id: ctftime_id.map(|id| id as u32),
        fields,
        created_at,
    })
}

pub(crate) fn map_challenge(row: &sqlx::postgres::PgRow) -> Result<Challenge, sqlx::Error> {
    let id: String = row.get("id");
    let title: String = row.get("title");
    let description: String = row.get("description");
    let category: String = row.get("category");
    let points_mode: String = row.get("points_mode");
    let points_equation: String = row.get("points_equation");
    let flag_val: serde_json::Value = row.get("flag");
    let author_id: String = row.get("author_id");
    let author_username: String = row.get("author_username");
    let hints_val: serde_json::Value = row.get("hints");
    let files_val: serde_json::Value = row.get("files");
    let tags_val: serde_json::Value = row.get("tags");
    let requirements_val: serde_json::Value = row.get("requirements");
    let team_consensus: bool = row.try_get("team_consensus").unwrap_or(false);
    let deployment_val: serde_json::Value = row
        .try_get("deployment")
        .unwrap_or_else(|_| serde_json::Value::String("None".to_string()));

    let mode = match points_mode.as_str() {
        "PointAttribution" => ScoringMode::PointAttribution,
        "DynamicDecay" => {
            let parts: Vec<&str> = points_equation.split(',').collect();
            if parts.len() == 3 {
                let initial = parts[0].parse::<u32>().unwrap_or(500);
                let minimum = parts[1].parse::<u32>().unwrap_or(100);
                let decay = parts[2].parse::<u32>().unwrap_or(10);
                ScoringMode::DynamicDecay {
                    initial,
                    minimum,
                    decay,
                }
            } else {
                ScoringMode::DynamicDecay {
                    initial: 500,
                    minimum: 100,
                    decay: 10,
                }
            }
        }
        _ => ScoringMode::PointValue,
    };
    let flag = serde_json::from_value(flag_val).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    let hints = serde_json::from_value(hints_val).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    let files = serde_json::from_value(files_val).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    let tags = serde_json::from_value(tags_val).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    let requirements =
        serde_json::from_value(requirements_val).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    let deployment =
        serde_json::from_value(deployment_val).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    let visibility_val: serde_json::Value = row
        .try_get("visibility")
        .unwrap_or_else(|_| serde_json::json!("Visible"));
    let visibility = serde_json::from_value(visibility_val)
        .unwrap_or(crate::libs::types::challenges::ChallengeVisibility::Visible);
    let max_attempts_val: Option<serde_json::Value> = row.try_get("max_attempts").ok().flatten();
    let max_attempts: Option<crate::libs::types::challenges::MaxAttempts> =
        max_attempts_val.and_then(|v| serde_json::from_value(v).ok());
    Ok(Challenge {
        id,
        title: crate::libs::types::challenges::ChallengeTitle(title),
        description: crate::libs::types::challenges::ChallengeDescription(
            crate::libs::types::htmlstring::HtmlString(description),
        ),
        category: crate::libs::types::challenges::ChallengeCategory(category),
        points: crate::libs::types::challenges::ChallengePoints {
            mode,
            equation: points_equation,
        },
        flag,
        author: crate::libs::types::challenges::ChallengeAuthor {
            id: author_id,
            username: author_username,
        },
        hints,
        files,
        tags,
        requirements,
        team_consensus,
        deployment,
        visibility,
        max_attempts,
    })
}

pub(crate) fn map_submission(row: &sqlx::postgres::PgRow) -> Result<Submission, sqlx::Error> {
    let id: String = row.get("id");
    let challenge_id: String = row.get("challenge_id");
    let team_id_str: Option<String> = row.get("team_id");
    let account_id: String = row.get("account_id");
    let points: i32 = row.get("points");
    let provided_flag: String = row.get("provided_flag");
    let is_correct: bool = row.get("is_correct");
    let submitted_at: i64 = row.get("submitted_at");
    let submitted_ip: String = row.get("submitted_ip");
    Ok(Submission {
        id: crate::libs::types::solves::SubmissionId(id),
        challenge_id,
        team_id: team_id_str.map(TeamId),
        account_id: AccountId(account_id),
        points: points as u32,
        provided_flag,
        is_correct,
        submitted_at,
        submitted_ip,
    })
}

pub(crate) fn map_hint_unlock(row: &sqlx::postgres::PgRow) -> Result<HintUnlock, sqlx::Error> {
    let id: String = row.get("id");
    let challenge_id: String = row.get("challenge_id");
    let hint_index: i32 = row.get("hint_index");
    let team_id_str: Option<String> = row.get("team_id");
    let account_id: String = row.get("account_id");
    let cost: i32 = row.get("cost");
    let unlocked_at: i64 = row.get("unlocked_at");
    Ok(HintUnlock {
        id: crate::libs::types::solves::HintUnlockId(id),
        challenge_id,
        hint_index: hint_index as u32,
        team_id: team_id_str.map(TeamId),
        account_id: AccountId(account_id),
        cost: cost as u32,
        unlocked_at,
    })
}
