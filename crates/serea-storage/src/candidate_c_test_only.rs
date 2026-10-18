//! Explicitly enabled Candidate C test-only grant-set revalidation.
//! This is a diagnostic read path. It does not authorize, consume, or mutate.
use crate::{ApprovalRequestActionDraft, StoreError, Tx, approval_action_set_digest};
use rusqlite::OptionalExtension;
use serea_protocol::{ApprovalId, Digest, GrantId, StepId, TaskId, digest_of};

pub(crate) fn probe_grant_full_set(
    tx: &Tx<'_>,
    grant_id: &GrantId,
    approval_id: &ApprovalId,
    task_id: &TaskId,
    selected_step: &StepId,
) -> Result<Vec<(i64, StepId, Digest, Digest)>, StoreError> {
    type GrantRequestIntegrityRow = (
        String,
        String,
        i64,
        String,
        String,
        i64,
        String,
        String,
        i64,
        i64,
        i64,
        String,
        String,
        String,
        i64,
        String,
        String,
        i64,
        String,
        String,
        i64,
    );
    let row: Option<GrantRequestIntegrityRow> = tx
        .inner
        .query_row(
            "SELECT g.approval_id,g.task_id,g.plan_revision,g.capability_id,
                g.capability_version,g.generation_id,g.descriptor_digest,
                g.action_set_digest,g.action_count,g.max_uses,g.uses_remaining,
                g.status,r.status,r.task_id,r.plan_revision,r.capability_id,
                r.capability_version,r.generation_id,r.descriptor_digest,
                r.action_set_digest,r.action_count
           FROM approval_grants g JOIN approval_requests r ON r.approval_id=g.approval_id
          WHERE g.grant_id=?1",
            [grant_id.as_str()],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                    r.get(12)?,
                    r.get(13)?,
                    r.get(14)?,
                    r.get(15)?,
                    r.get(16)?,
                    r.get(17)?,
                    r.get(18)?,
                    r.get(19)?,
                    r.get(20)?,
                ))
            },
        )
        .optional()?;
    let Some((
        g_approval,
        g_task,
        g_plan,
        g_cap,
        g_version,
        g_generation,
        g_descriptor,
        g_digest,
        g_count,
        g_max,
        g_remaining,
        g_status,
        r_status,
        r_task,
        r_plan,
        r_cap,
        r_version,
        r_generation,
        r_descriptor,
        r_digest,
        r_count,
    )) = row
    else {
        return Err(StoreError::InvalidApprovalResponse);
    };
    if g_approval != approval_id.as_str()
        || g_task != task_id.as_str()
        || r_status != "APPROVED"
        || r_task != g_task
        || r_plan != g_plan
        || r_cap != g_cap
        || r_version != g_version
        || r_generation != g_generation
        || r_descriptor != g_descriptor
        || !(1..=8).contains(&g_count)
        || r_count < g_count
        || g_max != g_count
        || g_remaining < 0
        || g_remaining > g_max
        || !matches!(
            g_status.as_str(),
            "ACTIVE" | "EXHAUSTED" | "REVOKED" | "EXPIRED"
        )
    {
        return Err(StoreError::InvalidApprovalResponse);
    }
    // The Request digest covers the complete requested set. Grant.action_set_digest
    // is a second digest over the approved subset, as apply_response currently
    // writes it. Recompute them independently; neither stored hash is keyed.
    let request_rows: Vec<(i64, String, String, String, String)> = {
        let mut statement = tx.inner.prepare(
            "SELECT position,step_id,arguments_digest,scope,scope_digest
               FROM approval_request_actions WHERE approval_id=?1 ORDER BY position ASC",
        )?;
        statement
            .query_map([approval_id.as_str()], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<Result<_, _>>()?
    };
    if request_rows.len() as i64 != r_count || request_rows.len() > 8 {
        return Err(StoreError::InvalidApprovalResponse);
    }
    let mut request_by_step = std::collections::BTreeMap::new();
    let mut previous_request_step: Option<String> = None;
    let mut request_action_set = Vec::with_capacity(request_rows.len());
    for (position, step, arguments, scope, scope_digest) in request_rows {
        let action = ApprovalRequestActionDraft {
            step_id: StepId::new(step).map_err(|_| StoreError::InvalidApprovalResponse)?,
            arguments_digest: Digest::new(arguments)
                .map_err(|_| StoreError::InvalidApprovalResponse)?,
            scope_json: scope,
            scope_digest: Digest::new(scope_digest)
                .map_err(|_| StoreError::InvalidApprovalResponse)?,
        };
        if position != request_by_step.len() as i64
            || previous_request_step
                .as_ref()
                .is_some_and(|prior| prior.as_bytes() >= action.step_id.as_str().as_bytes())
            || digest_of(&action.scope_json).map_err(|_| StoreError::InvalidApprovalResponse)?
                != action.scope_digest
            || request_by_step
                .insert(action.step_id.to_string(), action.clone())
                .is_some()
        {
            return Err(StoreError::InvalidApprovalResponse);
        }
        previous_request_step = Some(action.step_id.to_string());
        request_action_set.push(action);
    }
    if approval_action_set_digest(&request_action_set)
        .map_err(|_| StoreError::InvalidApprovalResponse)?
        .as_str()
        != r_digest
    {
        return Err(StoreError::InvalidApprovalResponse);
    }

    let seal_count: i64 = tx.inner.query_row(
        "SELECT count(*) FROM approval_grant_seals WHERE grant_id=?1",
        [grant_id.as_str()],
        |r| r.get(0),
    )?;
    if seal_count != 1 {
        return Err(StoreError::InvalidApprovalResponse);
    }
    let uses: Vec<(String, String)> = {
        let mut statement = tx.inner.prepare(
            "SELECT step_id,task_id FROM approval_grant_uses WHERE grant_id=?1 ORDER BY step_id",
        )?;
        statement
            .query_map([grant_id.as_str()], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    if uses.len() as i64 != g_max - g_remaining
        || (g_status == "ACTIVE" && g_remaining == 0)
        || (g_status == "EXHAUSTED" && g_remaining != 0)
    {
        return Err(StoreError::InvalidApprovalResponse);
    }

    let members: Vec<(i64, String, String, String, String)> = {
        let mut statement = tx.inner.prepare(
            "SELECT position,approval_id,step_id,arguments_digest,scope_digest
               FROM approval_grant_members WHERE grant_id=?1 ORDER BY position ASC",
        )?;
        statement
            .query_map([grant_id.as_str()], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<Result<_, _>>()?
    };
    if members.len() as i64 != g_count || members.len() > 8 {
        return Err(StoreError::InvalidApprovalResponse);
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut result = Vec::with_capacity(members.len());
    let mut grant_subset = Vec::with_capacity(members.len());
    let mut selected = false;
    for (
        expected_position,
        (position, member_approval, member_step, member_arguments, member_scope),
    ) in members.into_iter().enumerate()
    {
        if position != expected_position as i64
            || member_approval != approval_id.as_str()
            || !seen.insert(member_step.clone())
        {
            return Err(StoreError::InvalidApprovalResponse);
        }
        let step = StepId::new(member_step).map_err(|_| StoreError::InvalidApprovalResponse)?;
        let Some(action) = request_by_step.get(step.as_str()) else {
            return Err(StoreError::InvalidApprovalResponse);
        };
        if member_arguments != action.arguments_digest.as_str()
            || member_scope != action.scope_digest.as_str()
        {
            return Err(StoreError::InvalidApprovalResponse);
        }
        let step_facts: Option<(String, i64, String, String)> = tx
            .inner
            .query_row(
                "SELECT s.task_id,s.plan_revision,s.capability_id,s.capability_version
               FROM task_steps s WHERE s.step_id=?1",
                [step.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        if step_facts != Some((g_task.clone(), g_plan, g_cap.clone(), g_version.clone())) {
            return Err(StoreError::InvalidApprovalResponse);
        }
        let binding: Option<(i64, String)> = tx
            .inner
            .query_row(
                "SELECT generation_id,descriptor_digest FROM step_capability_bindings
              WHERE task_id=?1 AND step_id=?2",
                rusqlite::params![task_id.as_str(), step.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if binding != Some((g_generation, g_descriptor.clone())) {
            return Err(StoreError::InvalidApprovalResponse);
        }
        selected |= &step == selected_step;
        result.push((
            position,
            step,
            Digest::new(member_arguments).map_err(|_| StoreError::InvalidApprovalResponse)?,
            action.scope_digest.clone(),
        ));
        grant_subset.push(action.clone());
    }
    if !selected
        || uses
            .iter()
            .any(|(used_step, used_task)| used_task != &g_task || !seen.contains(used_step))
    {
        return Err(StoreError::InvalidApprovalResponse);
    }
    if approval_action_set_digest(&grant_subset)
        .map_err(|_| StoreError::InvalidApprovalResponse)?
        .as_str()
        != g_digest
    {
        return Err(StoreError::InvalidApprovalResponse);
    }
    Ok(result)
}
