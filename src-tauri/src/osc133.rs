//! Scanner for OSC 133 shell-integration sequences in a PTY byte stream.
//!
//! The scanner detects complete OSC 133 sequences across chunk boundaries and
//! emits the buffer back to the caller **serially**, split precisely at OSC 133
//! boundaries: each call produces an ordered stream of [`Segment`]s whose bytes
//! concatenate to the resolved portion of `carry + chunk`. Unrecognised bytes
//! (plain text, non-133 OSCs, unknown 133 markers) are never stripped — they
//! surface as [`Segment::Passthrough`] so the caller can forward them verbatim to
//! the terminal emulator. Each recognised OSC 133 sequence surfaces as a
//! [`Segment::Sequence`] carrying both its raw bytes (still to be forwarded so
//! xterm.js' OSC-133 parser hook can place positional decorations) and the
//! parsed shell-integration [`ShellEvent`].
//!
//! Bytes that may be the start of a sequence but cannot yet be resolved (e.g. an
//! OSC with no terminator yet) are retained in `carry` for the next chunk; they
//! are not emitted until the sequence either completes or is rejected.

/// OSC number for shell integration markers.
const OSC_SHELL_INTEGRATION: &[u8] = b"133;";

/// Maximum carry-buffer size. If exceeded, the oldest bytes are discarded to
/// prevent unbounded growth from a malformed or unterminated sequence.
const MAX_CARRY: usize = 512;

// ── Byte-level constants ──────────────────────────────────────────────

/// ESC — starts all escape sequences.
const ESC: u8 = 0x1B;
/// BEL — one of two OSC terminators.
const BEL: u8 = 0x07;
/// `]` — introduces an OSC after ESC.
const BRACKET: u8 = b']';
/// `\` — second byte of the ST (String Terminator) sequence ESC \.
const ST_SECOND: u8 = b'\\';

// ── OSC 133 parameter identifiers ─────────────────────────────────────

const MARKER_PROMPT: &[u8] = b"A";
const MARKER_PROMPT_END: &[u8] = b"B";
const MARKER_COMMAND: &[u8] = b"C";
const MARKER_FINISHED: &[u8] = b"D";

/// A recognised OSC 133 shell-integration event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellEvent {
    PromptStarted {
        aid: Option<String>,
    },
    PromptEnded {
        aid: Option<String>,
    },
    CommandStarted {
        aid: Option<String>,
    },
    CommandFinished {
        exit_code: Option<i32>,
        aid: Option<String>,
    },
}

/// A serial segment of the PTY byte stream, split at OSC 133 boundaries.
///
/// The segments returned by a single [`scan`] call, concatenated in order,
/// reproduce the resolved portion of `carry + chunk`. Bytes belonging to an
/// as-yet-unresolved sequence are retained in `carry` and absent from the
/// segments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Bytes that are not part of any recognised complete OSC 133 sequence.
    /// Includes plain text, non-133 OSCs, and 133 sequences with unknown
    /// markers. The caller should forward them verbatim to the terminal
    /// emulator.
    Passthrough(Vec<u8>),
    /// A complete, recognised OSC 133 sequence.
    Sequence {
        /// The raw sequence bytes (including `ESC ]`, the payload, and the
        /// terminator). Forward verbatim so xterm.js' parser hook can place
        /// decorations, then act on `event` for non-positional context.
        bytes: Vec<u8>,
        /// The parsed shell-integration event for this sequence.
        event: ShellEvent,
    },
}

/// Stream the resolved portion of `carry + chunk` to `emit` as ordered
/// [`Segment`]s split at OSC 133 boundaries.
///
/// Bytes that may begin a sequence but cannot yet be resolved are retained in
/// `carry` for the next call. Already-resolved bytes are drained from `carry`
/// on return, so `carry` only ever holds an unresolved tail (plus up to
/// [`MAX_CARRY`] bytes of overflow protection).
///
/// Concatenating the `bytes` of every emitted segment yields exactly the
/// resolved portion of the input — nothing is added, stripped, or reordered.
pub fn scan(carry: &mut Vec<u8>, chunk: &[u8], mut emit: impl FnMut(Segment)) {
    carry.extend_from_slice(chunk);

    // Offset of the first byte not yet committed to a segment. Only advances
    // when we emit a recognised OSC 133 sequence (or during the final flush),
    // so non-133 bytes naturally fold into the next passthrough segment.
    let mut pending = 0usize;
    let mut pos = 0usize;

    'outer: while pos < carry.len() {
        // Fast skip: advance past bytes that cannot start an escape.
        if carry[pos] != ESC {
            pos += 1;
            continue;
        }

        // ESC at end of buffer – could be start of ESC ] …; keep it for next
        // chunk. Bytes before it are safe to flush below.
        if pos + 1 >= carry.len() {
            break;
        }

        // ESC followed by something other than ] — not an OSC.
        if carry[pos + 1] != BRACKET {
            pos += 2;
            continue;
        }

        // ESC ] found — scan forward for the terminator (BEL or ST).
        let content_start = pos + 2;
        let mut k = content_start;

        while k < carry.len() {
            if carry[k] == BEL {
                let end = k + 1;
                emit_sequence(&carry, pos, content_start, k, end, &mut pending, &mut emit);
                pos = end;
                continue 'outer;
            }

            if carry[k] == ESC {
                if k + 1 < carry.len() {
                    if carry[k + 1] == ST_SECOND {
                        let end = k + 2;
                        emit_sequence(&carry, pos, content_start, k, end, &mut pending, &mut emit);
                        pos = end;
                        continue 'outer;
                    }
                    // ESC followed by something other than \ — not ST; keep
                    // scanning for a terminator further on. The inner ESC may
                    // yet turn out to start a real terminator.
                } else {
                    // Incomplete potential ST; wait for more data.
                    break;
                }
            }

            k += 1;
        }

        // No terminator found yet — incomplete sequence; stop scanning.
        break;
    }

    // Flush any safe text accumulated before the current scan position. Bytes
    // in [pending .. pos) are guaranteed not to be part of an incomplete
    // sequence: they are either plain text or complete non-133 OSCs that the
    // scan advanced past without emitting a `Sequence`.
    if pos > pending {
        emit(Segment::Passthrough(carry[pending..pos].to_vec()));
        pending = pos;
    }

    debug_assert_eq!(pending, pos);

    // Retain the unresolved tail [pos .. len) for the next chunk.
    if pos > 0 {
        carry.drain(..pos);
    }

    // Safety valve: discard old carry data to prevent unbounded growth from a
    // malformed or unterminated sequence.
    if carry.len() > MAX_CARRY {
        carry.drain(..carry.len() - MAX_CARRY);
    }
}

/// If the OSC spanning `carry[seq_start .. end]` (content between
/// `content_start` and `terminator_start`) is a recognised OSC 133 sequence,
/// flush any pending passthrough text before it, then emit a [`Segment::Sequence`].
///
/// For non-133 OSCs (including unknown 133 markers) nothing is emitted and
/// `pending` is left untouched — those bytes remain in the pending range and
/// will be folded into a later passthrough segment.
fn emit_sequence(
    carry: &[u8],
    seq_start: usize,
    content_start: usize,
    terminator_start: usize,
    end: usize,
    pending: &mut usize,
    emit: &mut impl FnMut(Segment),
) {
    let osc = &carry[content_start..terminator_start];
    let Some(rest) = osc.strip_prefix(OSC_SHELL_INTEGRATION) else {
        return;
    };
    let Some(event) = parse_event(rest) else {
        return;
    };

    // Flush the text accumulated before this sequence as a single passthrough
    // segment so caller-relative byte order is preserved.
    if seq_start > *pending {
        emit(Segment::Passthrough(carry[*pending..seq_start].to_vec()));
    }
    emit(Segment::Sequence {
        bytes: carry[seq_start..end].to_vec(),
        event,
    });
    *pending = end;
}

/// Try to extract an `aid=<value>` parameter from a slice of semicolon-separated
/// parameter segments. Returns the aid string if found.
fn parse_aid(params: &[&[u8]]) -> Option<String> {
    for param in params {
        if let Some(aid_val) = param.strip_prefix(b"aid=") {
            return std::str::from_utf8(aid_val).ok().map(|s| s.to_string());
        }
    }
    None
}

/// Translate the parameter portion of an OSC 133 sequence (everything after the
/// `133;` prefix) into a [`ShellEvent`].
///
/// The parameter format is `<marker>[;<param>...]` where `<param>` is either
/// an exit code (for `D`) or a `key=value` pair like `aid=<id>`. Unknown
/// markers yield `None`.
fn parse_event(param: &[u8]) -> Option<ShellEvent> {
    let mut parts = param.split(|&b| b == b';');

    let marker = parts.next()?;

    // Collect the remaining semicolon-separated segments.
    let remainder: Vec<&[u8]> = parts.collect();

    let aid = parse_aid(&remainder);

    match marker {
        MARKER_PROMPT => Some(ShellEvent::PromptStarted { aid }),
        MARKER_PROMPT_END => Some(ShellEvent::PromptEnded { aid }),
        MARKER_COMMAND => Some(ShellEvent::CommandStarted { aid }),
        MARKER_FINISHED => {
            // For D, the first non-aid parameter (if any) is the exit code.
            let exit_code = remainder.iter().find_map(|p| {
                if p.strip_prefix(b"aid=").is_some() {
                    None // skip aid= params
                } else {
                    std::str::from_utf8(p).ok().and_then(|s| s.parse().ok())
                }
            });
            Some(ShellEvent::CommandFinished { exit_code, aid })
        }
        _ => None, // unknown marker
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run `scan` and collect every emitted segment in order.
    fn scan_collect(carry: &mut Vec<u8>, chunk: &[u8]) -> Vec<Segment> {
        let mut segments = Vec::new();
        scan(carry, chunk, |s| segments.push(s));
        segments
    }

    /// Convenience: extract just the shell events from a segment list.
    fn events(segments: &[Segment]) -> Vec<ShellEvent> {
        segments
            .iter()
            .filter_map(|s| match s {
                Segment::Sequence { event, .. } => Some(event.clone()),
                Segment::Passthrough(_) => None,
            })
            .collect()
    }

    /// Convenience: concatenate every segment's bytes (passthrough + sequence).
    fn joined_bytes(segments: &[Segment]) -> Vec<u8> {
        let mut out = Vec::new();
        for s in segments {
            match s {
                Segment::Passthrough(b) => out.extend_from_slice(b),
                Segment::Sequence { bytes, .. } => out.extend_from_slice(bytes),
            }
        }
        out
    }

    #[test]
    fn single_bel_terminated_prompt() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;A\x07";
        let segments = scan_collect(&mut carry, seq);
        assert_eq!(
            segments,
            vec![Segment::Sequence {
                bytes: seq.to_vec(),
                event: ShellEvent::PromptStarted { aid: None },
            }]
        );
        assert!(carry.is_empty());
    }

    #[test]
    fn single_st_terminated_command() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;C\x1b\\";
        let segments = scan_collect(&mut carry, seq);
        assert_eq!(
            segments,
            vec![Segment::Sequence {
                bytes: seq.to_vec(),
                event: ShellEvent::CommandStarted { aid: None },
            }]
        );
        assert!(carry.is_empty());
    }

    #[test]
    fn finished_with_exit_code() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;D;0\x07";
        let segments = scan_collect(&mut carry, seq);
        assert_eq!(
            segments,
            vec![Segment::Sequence {
                bytes: seq.to_vec(),
                event: ShellEvent::CommandFinished {
                    exit_code: Some(0),
                    aid: None
                },
            }]
        );
        assert!(carry.is_empty());
    }

    #[test]
    fn finished_without_exit_code() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;D\x07";
        let segments = scan_collect(&mut carry, seq);
        assert_eq!(
            segments,
            vec![Segment::Sequence {
                bytes: seq.to_vec(),
                event: ShellEvent::CommandFinished {
                    exit_code: None,
                    aid: None
                },
            }]
        );
        assert!(carry.is_empty());
    }

    #[test]
    fn sequence_split_across_chunks() {
        let mut carry = Vec::new();
        let part1 = b"\x1b]133;";
        let part2 = b"A\x07";

        let first = scan_collect(&mut carry, part1);
        assert!(first.is_empty(), "no segments until the sequence completes");
        assert_eq!(carry, *part1);

        let second = scan_collect(&mut carry, part2);
        assert_eq!(
            second,
            vec![Segment::Sequence {
                bytes: b"\x1b]133;A\x07".to_vec(),
                event: ShellEvent::PromptStarted { aid: None },
            }]
        );
        assert!(carry.is_empty());
    }

    #[test]
    fn non_osc133_sequences_become_passthrough() {
        let mut carry = Vec::new();
        let seq = b"\x1b]0;title\x07some text\x1b]133;A\x07";
        let segments = scan_collect(&mut carry, seq);
        assert_eq!(
            segments,
            vec![
                Segment::Passthrough(b"\x1b]0;title\x07some text".to_vec()),
                Segment::Sequence {
                    bytes: b"\x1b]133;A\x07".to_vec(),
                    event: ShellEvent::PromptStarted { aid: None },
                },
            ]
        );
        assert_eq!(
            events(&segments),
            vec![ShellEvent::PromptStarted { aid: None }]
        );
        assert!(carry.is_empty());
    }

    #[test]
    fn mixed_events_in_one_chunk() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;A\x07\x1b]133;C\x07\x1b]133;D;127\x07";
        let segments = scan_collect(&mut carry, seq);
        assert_eq!(
            segments,
            vec![
                Segment::Sequence {
                    bytes: b"\x1b]133;A\x07".to_vec(),
                    event: ShellEvent::PromptStarted { aid: None },
                },
                Segment::Sequence {
                    bytes: b"\x1b]133;C\x07".to_vec(),
                    event: ShellEvent::CommandStarted { aid: None },
                },
                Segment::Sequence {
                    bytes: b"\x1b]133;D;127\x07".to_vec(),
                    event: ShellEvent::CommandFinished {
                        exit_code: Some(127),
                        aid: None
                    },
                },
            ]
        );
        // No passthrough segments: the chunk is purely three sequences.
        assert!(
            segments
                .iter()
                .all(|s| matches!(s, Segment::Sequence { .. }))
        );
        assert!(carry.is_empty());
    }

    #[test]
    fn esc_at_chunk_boundary_flushes_preceding_text_early() {
        let mut carry = Vec::new();
        let part1 = b"hello\x1b";
        let part2 = b"]133;A\x07";

        let first = scan_collect(&mut carry, part1);
        // The trailing ESC could start a sequence, so "hello" is safe text and
        // is flushed now; the ESC is retained.
        assert_eq!(first, vec![Segment::Passthrough(b"hello".to_vec())]);
        assert_eq!(carry, b"\x1b");

        let second = scan_collect(&mut carry, part2);
        assert_eq!(
            second,
            vec![Segment::Sequence {
                bytes: b"\x1b]133;A\x07".to_vec(),
                event: ShellEvent::PromptStarted { aid: None },
            }]
        );
        assert!(carry.is_empty());
    }

    #[test]
    fn plain_garbage_is_flushed_not_retained() {
        let mut carry = Vec::new();
        let garbage = vec![b'X'; 600];
        let segments = scan_collect(&mut carry, &garbage);
        assert_eq!(segments, vec![Segment::Passthrough(garbage.clone())]);
        assert!(carry.is_empty());
    }

    #[test]
    fn max_carry_safety_valve_on_unterminated_osc() {
        let mut carry = Vec::new();
        // A single OSC opened but never terminated, padded well beyond MAX_CARRY.
        let mut chunk = b"\x1b]133;A".to_vec();
        chunk.extend(std::iter::repeat(b'X').take(MAX_CARRY * 2));

        let segments = scan_collect(&mut carry, &chunk);
        // The sequence never terminates, so nothing is resolved/emitted.
        assert!(segments.is_empty());
        // Safety valve kicks in: carry is bounded to MAX_CARRY.
        assert_eq!(carry.len(), MAX_CARRY, "carry.len() = {}", carry.len());
        // The valve discards the OLDEST bytes: the leading ESC ] 133 ; A is
        // gone, and the surviving bytes are the newest MAX_CARRY X's.
        assert!(
            carry.iter().all(|&b| b == b'X'),
            "carry should be all X: {:?}",
            carry
        );
    }

    #[test]
    fn prompt_with_aid() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;A;aid=42\x07";
        let segments = scan_collect(&mut carry, seq);
        assert_eq!(
            segments,
            vec![Segment::Sequence {
                bytes: seq.to_vec(),
                event: ShellEvent::PromptStarted {
                    aid: Some("42".to_string())
                },
            }]
        );
    }

    #[test]
    fn prompt_with_pid_counter_aid() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;A;aid=12345-0\x07";
        let segments = scan_collect(&mut carry, seq);
        assert_eq!(
            segments,
            vec![Segment::Sequence {
                bytes: seq.to_vec(),
                event: ShellEvent::PromptStarted {
                    aid: Some("12345-0".to_string())
                },
            }]
        );
    }

    #[test]
    fn command_with_aid() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;C;aid=7\x07";
        let segments = scan_collect(&mut carry, seq);
        assert_eq!(
            segments,
            vec![Segment::Sequence {
                bytes: seq.to_vec(),
                event: ShellEvent::CommandStarted {
                    aid: Some("7".to_string())
                },
            }]
        );
    }

    #[test]
    fn finished_with_exit_code_and_aid() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;D;0;aid=3\x07";
        let segments = scan_collect(&mut carry, seq);
        assert_eq!(
            segments,
            vec![Segment::Sequence {
                bytes: seq.to_vec(),
                event: ShellEvent::CommandFinished {
                    exit_code: Some(0),
                    aid: Some("3".to_string())
                },
            }]
        );
    }

    #[test]
    fn finished_with_aid_only() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;D;aid=5\x07";
        let segments = scan_collect(&mut carry, seq);
        assert_eq!(
            segments,
            vec![Segment::Sequence {
                bytes: seq.to_vec(),
                event: ShellEvent::CommandFinished {
                    exit_code: None,
                    aid: Some("5".to_string())
                },
            }]
        );
    }

    #[test]
    fn prompt_end_marker() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;B;aid=1\x07";
        let segments = scan_collect(&mut carry, seq);
        assert_eq!(
            segments,
            vec![Segment::Sequence {
                bytes: seq.to_vec(),
                event: ShellEvent::PromptEnded {
                    aid: Some("1".to_string())
                },
            }]
        );
    }

    #[test]
    fn full_command_cycle_with_aid() {
        let mut carry = Vec::new();
        let seq =
            b"\x1b]133;A;aid=12345-1\x07\x1b]133;B;aid=12345-1\x07\x1b]133;C;aid=12345-1\x07\x1b]133;D;0;aid=12345-1\x07";
        let segments = scan_collect(&mut carry, seq);
        assert_eq!(
            segments,
            vec![
                Segment::Sequence {
                    bytes: b"\x1b]133;A;aid=12345-1\x07".to_vec(),
                    event: ShellEvent::PromptStarted {
                        aid: Some("12345-1".to_string())
                    },
                },
                Segment::Sequence {
                    bytes: b"\x1b]133;B;aid=12345-1\x07".to_vec(),
                    event: ShellEvent::PromptEnded {
                        aid: Some("12345-1".to_string())
                    },
                },
                Segment::Sequence {
                    bytes: b"\x1b]133;C;aid=12345-1\x07".to_vec(),
                    event: ShellEvent::CommandStarted {
                        aid: Some("12345-1".to_string())
                    },
                },
                Segment::Sequence {
                    bytes: b"\x1b]133;D;0;aid=12345-1\x07".to_vec(),
                    event: ShellEvent::CommandFinished {
                        exit_code: Some(0),
                        aid: Some("12345-1".to_string())
                    },
                },
            ]
        );
        assert!(carry.is_empty());
    }

    #[test]
    fn serial_segments_concatenate_to_resolved_input() {
        // Text and sequences interleaved; the concatenation of all segments
        // must equal the resolved input byte-for-byte.
        let mut carry = Vec::new();
        let input = b"boot text\x1b]133;A;aid=1\x07prompt$ ls\x1b]133;C\x07ls out\x1b]133;D;0\x07";
        let segments = scan_collect(&mut carry, &input[..]);
        assert!(carry.is_empty());
        assert_eq!(joined_bytes(&segments), &input[..]);
    }

    #[test]
    fn serial_split_preserves_order_across_chunks() {
        let mut carry = Vec::new();
        let p1 = b"boot ";
        let p2 = b"text\x1b]133;A";
        let p3 = b";aid=9\x07prompt$ ";

        let s1 = scan_collect(&mut carry, p1);
        assert_eq!(s1, vec![Segment::Passthrough(b"boot ".to_vec())]);
        assert!(carry.is_empty());

        let s2 = scan_collect(&mut carry, p2);
        // "text" is safe and flushed; the opening of the OSC is retained.
        assert_eq!(s2, vec![Segment::Passthrough(b"text".to_vec())]);
        assert_eq!(carry, b"\x1b]133;A");

        let s3 = scan_collect(&mut carry, p3);
        assert_eq!(
            s3,
            vec![
                Segment::Sequence {
                    bytes: b"\x1b]133;A;aid=9\x07".to_vec(),
                    event: ShellEvent::PromptStarted {
                        aid: Some("9".to_string())
                    },
                },
                Segment::Passthrough(b"prompt$ ".to_vec()),
            ]
        );
        assert!(carry.is_empty());

        // Reconstruct the original stream from the serial segments.
        let mut all = Vec::new();
        for s in s1.iter().chain(s2.iter()).chain(s3.iter()) {
            match s {
                Segment::Passthrough(b) => all.extend_from_slice(b),
                Segment::Sequence { bytes, .. } => all.extend_from_slice(bytes),
            }
        }
        let mut expected = Vec::new();
        for p in [p1.as_slice(), p2.as_slice(), p3.as_slice()] {
            expected.extend_from_slice(p);
        }
        assert_eq!(all, expected);
    }

    #[test]
    fn unknown_133_marker_folds_into_passthrough() {
        let mut carry = Vec::new();
        let seq = b"\x1b]133;Z;aid=1\x07after\x1b]133;A\x07";
        let segments = scan_collect(&mut carry, seq);
        assert_eq!(
            segments,
            vec![
                Segment::Passthrough(b"\x1b]133;Z;aid=1\x07after".to_vec()),
                Segment::Sequence {
                    bytes: b"\x1b]133;A\x07".to_vec(),
                    event: ShellEvent::PromptStarted { aid: None },
                },
            ]
        );
    }
}
