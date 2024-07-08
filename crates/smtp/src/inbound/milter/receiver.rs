/*
 * Copyright (c) 2023 Stalwart Labs Ltd.
 *
 * This file is part of Stalwart Mail Server.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
*/

use std::borrow::Cow;

enum State {
    Len {
        buf: [u8; std::mem::size_of::<u32>()],
        bytes_read: usize,
    },
    Frame {
        buf: Vec<u8>,
        frame_len: usize,
    },
}

pub struct Receiver {
    packet_pos: usize,
    state: State,
    max_frame_len: usize,
}

pub enum FrameResult<'x> {
    Frame(Cow<'x, [u8]>),
    Incomplete,
    TooLarge(usize),
}

impl Default for State {
    fn default() -> Self {
        State::Len {
            buf: [0; std::mem::size_of::<u32>()],
            bytes_read: 0,
        }
    }
}

impl Receiver {
    pub fn with_max_frame_len(max_frame_len: usize) -> Self {
        Receiver {
            packet_pos: 0,
            state: State::default(),
            max_frame_len,
        }
    }

    pub fn read_frame<'x>(&mut self, packet: &'x [u8]) -> FrameResult<'x> {
        if !packet.is_empty() {
            match &mut self.state {
                State::Len { buf, bytes_read } => {
                    while *bytes_read < std::mem::size_of::<u32>() {
                        if let Some(byte) = packet.get(self.packet_pos) {
                            buf[*bytes_read] = *byte;
                            *bytes_read += 1;
                            self.packet_pos += 1;
                        } else {
                            self.packet_pos = 0;
                            return FrameResult::Incomplete;
                        }
                    }
                    let length = u32::from_be_bytes(*buf) as usize;
                    if length <= self.max_frame_len {
                        if let Some(frame) = packet.get(self.packet_pos..self.packet_pos + length) {
                            self.packet_pos += length;
                            self.state = State::default();
                            FrameResult::Frame(frame.into())
                        } else {
                            let mut buf = Vec::with_capacity(length);
                            if let Some(bytes_available) = packet.get(self.packet_pos..) {
                                buf.extend(bytes_available);
                            }
                            self.state = State::Frame {
                                buf,
                                frame_len: length,
                            };
                            self.packet_pos = 0;
                            FrameResult::Incomplete
                        }
                    } else {
                        FrameResult::TooLarge(length)
                    }
                }
                State::Frame { buf, frame_len } => {
                    let bytes_pending = *frame_len - buf.len();
                    if let Some(bytes) =
                        packet.get(self.packet_pos..self.packet_pos + bytes_pending)
                    {
                        let mut buf = std::mem::take(buf);
                        buf.extend(bytes);
                        self.packet_pos += bytes_pending;
                        self.state = State::default();
                        FrameResult::Frame(buf.into())
                    } else if let Some(bytes_available) = packet.get(self.packet_pos..) {
                        buf.extend(bytes_available);
                        self.packet_pos = 0;
                        FrameResult::Incomplete
                    } else {
                        self.packet_pos = 0;
                        FrameResult::Incomplete
                    }
                }
            }
        } else {
            FrameResult::Incomplete
        }
    }
}
