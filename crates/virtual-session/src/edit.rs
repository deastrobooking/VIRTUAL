//! Assemble trimmed control takes without modifying the source journals.
use crate::{
    CommandOrigin, JournalRecovery, SessionError, ShowTime, StateCheckpoint, TimelineMarker,
};

/// A source journal's half-open interval `(in_ns, out_ns]`. The state at
/// `in_ns` becomes a checkpoint at the cut, including commands on that boundary.
pub struct TakeSlice<'a> {
    pub journal: &'a JournalRecovery,
    pub in_ns: u64,
    pub out_ns: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("Select between one and 128 take segments")]
    SegmentCount,
    #[error("Take segment {0} has an invalid range or no baseline at its In point")]
    Range(usize),
    #[error("Take segments must belong to the same project")]
    ProjectMismatch,
    #[error("Edited timeline is too long")]
    Overflow,
    #[error(transparent)]
    Replay(#[from] SessionError),
}

/// Rebase command identities and time at every cut, and carry the exact
/// source state into the next segment. Original origins remain in source files.
pub fn assemble_takes(name: &str, slices: &[TakeSlice<'_>]) -> Result<JournalRecovery, EditError> {
    if slices.is_empty() || slices.len() > 128 {
        return Err(EditError::SegmentCount);
    }
    let project_id = slices[0].journal.project_id.clone();
    let mut output = JournalRecovery {
        take_name: name.to_owned(),
        project_id: project_id.clone(),
        ..Default::default()
    };
    let mut offset = 0u64;
    let mut sequence = 0u64;
    for (index, slice) in slices.iter().enumerate() {
        if slice.journal.project_id != project_id {
            return Err(EditError::ProjectMismatch);
        }
        let first_checkpoint = slice
            .journal
            .history_checkpoints
            .iter()
            .chain(slice.journal.checkpoint.iter())
            .map(|checkpoint| checkpoint.at.monotonic_ns)
            .min();
        if slice.in_ns >= slice.out_ns
            || slice.out_ns > slice.journal.latest_time().monotonic_ns
            || first_checkpoint.is_none_or(|first| slice.in_ns < first)
        {
            return Err(EditError::Range(index + 1));
        }
        let end = offset
            .checked_add(slice.out_ns - slice.in_ns)
            .ok_or(EditError::Overflow)?;
        let time = |ns| ShowTime {
            monotonic_ns: ns,
            ..Default::default()
        };
        let mut baseline = slice.journal.replay_at(time(slice.in_ns))?;
        baseline.last_sequence = sequence.checked_sub(1);
        output.history_checkpoints.push(StateCheckpoint {
            after_sequence: baseline.last_sequence,
            at: time(offset),
            state: baseline,
        });
        output.markers.push(TimelineMarker {
            at: time(offset),
            label: format!("Cut {}", index + 1),
        });
        let segment_sequence = sequence;
        for command in slice.journal.history_commands.iter().filter(|command| {
            command.execute_at.monotonic_ns > slice.in_ns
                && command.execute_at.monotonic_ns <= slice.out_ns
        }) {
            let mut command = command.clone();
            command.sequence = sequence;
            command.command_id = sequence;
            command.origin = CommandOrigin::Replay;
            command.execute_at = time(offset + command.execute_at.monotonic_ns - slice.in_ns);
            sequence = sequence.checked_add(1).ok_or(EditError::Overflow)?;
            output.history_commands.push(command);
        }
        for marker in slice.journal.markers.iter().filter(|marker| {
            marker.at.monotonic_ns > slice.in_ns && marker.at.monotonic_ns <= slice.out_ns
        }) {
            output.markers.push(TimelineMarker {
                at: time(offset + marker.at.monotonic_ns - slice.in_ns),
                label: marker.label.clone(),
            });
        }
        for checkpoint in slice
            .journal
            .history_checkpoints
            .iter()
            .chain(slice.journal.checkpoint.iter())
            .filter(|checkpoint| {
                checkpoint.at.monotonic_ns > slice.in_ns
                    && checkpoint.at.monotonic_ns < slice.out_ns
            })
        {
            let count = slice
                .journal
                .history_commands
                .iter()
                .filter(|command| {
                    command.execute_at.monotonic_ns > slice.in_ns
                        && command.execute_at.monotonic_ns <= checkpoint.at.monotonic_ns
                        && checkpoint
                            .after_sequence
                            .is_some_and(|after| command.sequence <= after)
                })
                .count() as u64;
            let after_sequence = (segment_sequence + count).checked_sub(1);
            let mut state = checkpoint.state.clone();
            state.last_sequence = after_sequence;
            output.history_checkpoints.push(StateCheckpoint {
                after_sequence,
                at: time(offset + checkpoint.at.monotonic_ns - slice.in_ns),
                state,
            });
        }
        let mut final_state = slice.journal.replay_at(time(slice.out_ns))?;
        final_state.last_sequence = sequence.checked_sub(1);
        output.history_checkpoints.push(StateCheckpoint {
            after_sequence: final_state.last_sequence,
            at: time(end),
            state: final_state,
        });
        offset = end;
    }
    output.checkpoint = output.history_checkpoints.last().cloned();
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandOperation, SessionState, ShowCommand};
    fn source() -> JournalRecovery {
        let baseline = StateCheckpoint {
            at: ShowTime::default(),
            after_sequence: None,
            state: SessionState::default(),
        };
        JournalRecovery {
            project_id: Some("a".repeat(32)),
            history_checkpoints: vec![baseline],
            history_commands: vec![
                ShowCommand {
                    sequence: 0,
                    command_id: 10,
                    origin: CommandOrigin::Operator,
                    execute_at: ShowTime {
                        monotonic_ns: 10,
                        ..Default::default()
                    },
                    operation: CommandOperation::SetTempo { bpm: 140.0 },
                },
                ShowCommand {
                    sequence: 1,
                    command_id: 11,
                    origin: CommandOrigin::Operator,
                    execute_at: ShowTime {
                        monotonic_ns: 20,
                        ..Default::default()
                    },
                    operation: CommandOperation::SetBlackout { enabled: true },
                },
            ],
            ..Default::default()
        }
    }
    #[test]
    fn trimmed_join_rebases_identity_and_restores_state_at_each_cut() {
        let journal = source();
        let edited = assemble_takes(
            "Edit",
            &[
                TakeSlice {
                    journal: &journal,
                    in_ns: 10,
                    out_ns: 20,
                },
                TakeSlice {
                    journal: &journal,
                    in_ns: 0,
                    out_ns: 20,
                },
            ],
        )
        .unwrap();
        assert_eq!(
            edited
                .history_commands
                .iter()
                .map(|command| command.sequence)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(edited.latest_time().monotonic_ns, 30);
        let at = |ns| {
            edited
                .replay_at(ShowTime {
                    monotonic_ns: ns,
                    ..Default::default()
                })
                .unwrap()
        };
        assert_eq!(at(0).bpm, 140.0);
        assert!(!at(10).blackout, "next source baseline wins at a cut");
        assert_eq!(at(10).bpm, 120.0);
        assert!(at(30).blackout);
        assert!(edited.replay_state().unwrap().blackout);
        assert_eq!(journal.history_commands[0].command_id, 10);
    }
    #[test]
    fn refuses_out_of_range_and_foreign_project_slices() {
        let journal = source();
        assert!(
            assemble_takes(
                "Edit",
                &[TakeSlice {
                    journal: &journal,
                    in_ns: 20,
                    out_ns: 21
                }]
            )
            .is_err()
        );
        let mut foreign = source();
        foreign.project_id = Some("b".repeat(32));
        assert!(matches!(
            assemble_takes(
                "Edit",
                &[
                    TakeSlice {
                        journal: &journal,
                        in_ns: 0,
                        out_ns: 20
                    },
                    TakeSlice {
                        journal: &foreign,
                        in_ns: 0,
                        out_ns: 20
                    },
                ]
            ),
            Err(EditError::ProjectMismatch)
        ));
    }

    #[test]
    fn intermediate_checkpoints_keep_dynamic_state_inside_trimmed_segments() {
        let mut journal = source();
        journal.history_checkpoints.push(StateCheckpoint {
            at: ShowTime {
                monotonic_ns: 15,
                ..Default::default()
            },
            after_sequence: Some(0),
            state: SessionState {
                bpm: 140.0,
                deck_positions: [7.0, 0.0, 0.0, 0.0],
                last_sequence: Some(0),
                ..Default::default()
            },
        });
        let edited = assemble_takes(
            "Trim",
            &[TakeSlice {
                journal: &journal,
                in_ns: 5,
                out_ns: 20,
            }],
        )
        .unwrap();
        let state = edited
            .replay_at(ShowTime {
                monotonic_ns: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(state.deck_positions[0], 7.0);
        assert_eq!(state.bpm, 140.0);
        assert!(!state.blackout);
        assert!(edited.replay_state().unwrap().blackout);
    }
}
