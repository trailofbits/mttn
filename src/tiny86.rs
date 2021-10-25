use std::io::Write;

use anyhow::{anyhow, Result};

use crate::trace::{MemoryHint, RegisterFile, Step};

const TINY86_MAX_INSTR_LEN: usize = 12;
const TINY86_MAX_HINT_DATA_LEN: usize = 4;
const TINY86_MAX_HINTS: usize = 2;

pub trait Tiny86Write {
    fn serialized_size() -> usize;
    fn pad_write(w: &mut impl Write) -> Result<()>;
    fn tiny86_write(&self, w: &mut impl Write) -> Result<()>;
}

pub trait Bitstring {
    fn bitstring(&self) -> Result<String>;
}

/// A Tiny86 memory hint is serialized as three fields, in order:
///
/// 0. Operation mask and width (1 byte)
/// 1. Address (4 bytes)
/// 2. Data (4 bytes)
impl Tiny86Write for MemoryHint {
    fn serialized_size() -> usize {
        9
    }

    fn pad_write(w: &mut impl Write) -> Result<()> {
        // NOTE(ww): This would be better as an array, but then `serialized_size()` would have
        // to be a `const fn` and Rust (as of 1.48) doesn't support those in traits or trait
        // impls yet.
        let nothing = vec![0u8; Self::serialized_size()];
        w.write_all(&nothing)?;

        Ok(())
    }

    fn tiny86_write(&self, w: &mut impl Write) -> Result<()> {
        // Our memory mask and operation are packed into a single byte, as follows:
        //
        // |  7     6     5     4     3  |  2  |  1     0    |
        // |==================================================
        // |  1  |    reserved           | r/w |    mask     |
        // |=================================================|
        // |  7     6     5     4     3  |  2  |  1     0    |
        //
        // The high bit is always 1, to indicate a valid memory hint.
        let mut packed: u8 = self.mask as u8;
        packed |= ((self.operation as u8) << 2) | 0x80;

        w.write_all(&[packed])?;
        w.write_all(&(self.address as u32).to_be_bytes())?;

        if self.data.len() > TINY86_MAX_HINT_DATA_LEN {
            return Err(anyhow!(
                "invariant failure: data len {} > {}",
                self.data.len(),
                TINY86_MAX_HINT_DATA_LEN
            ));
        }

        // We reserve 4 bytes for the hint's data, but we could have less.
        // Any remaining bytes are zeroes.
        let mut data = vec![0u8; TINY86_MAX_HINT_DATA_LEN];
        data.splice(..self.data.len(), self.data.iter().cloned());

        w.write_all(&data)?;

        Ok(())
    }
}

/// A Tiny86 register file is serialized as 10 fields:
///
/// * 8 GPRs (each 4 bytes)
/// * EIP (4 bytes)
/// * EFLAGS (4 bytes)
impl Tiny86Write for RegisterFile {
    fn serialized_size() -> usize {
        40
    }

    fn pad_write(w: &mut impl Write) -> Result<()> {
        let nothing = vec![0u8; Self::serialized_size()];
        w.write_all(&nothing)?;

        Ok(())
    }

    fn tiny86_write(&self, w: &mut impl Write) -> Result<()> {
        // GPRs.
        w.write_all(&(self.rax as u32).to_be_bytes())?;
        w.write_all(&(self.rbx as u32).to_be_bytes())?;
        w.write_all(&(self.rcx as u32).to_be_bytes())?;
        w.write_all(&(self.rdx as u32).to_be_bytes())?;
        w.write_all(&(self.rsi as u32).to_be_bytes())?;
        w.write_all(&(self.rdi as u32).to_be_bytes())?;
        w.write_all(&(self.rsp as u32).to_be_bytes())?;
        w.write_all(&(self.rbp as u32).to_be_bytes())?;

        // EIP and EFLAGS.
        w.write_all(&(self.rip as u32).to_be_bytes())?;
        w.write_all(&(self.rflags as u32).to_be_bytes())?;

        Ok(())
    }
}

/// A Tiny86 trace step is serialized as:
///
/// * The raw instruction bytes (padded out to a maximum of 12 using NOPs)
/// * The register file
/// * Two memory hints (one or more of which may be blank)
///
/// Observe that the order of serialization below is the reverse of the above,
/// since these traces are consumed as bits starting with the instruction
/// at bit 0. Observe also that multi-byte fields are in big-endian order,
/// since that's what the circuit uses internally.
impl Tiny86Write for Step {
    fn serialized_size() -> usize {
        TINY86_MAX_INSTR_LEN + RegisterFile::serialized_size() + (MemoryHint::serialized_size() * 2)
    }

    fn pad_write(w: &mut impl Write) -> Result<()> {
        MemoryHint::pad_write(w)?;
        MemoryHint::pad_write(w)?;

        RegisterFile::pad_write(w)?;

        let nothing = [0x90u8; TINY86_MAX_INSTR_LEN];
        w.write_all(&nothing)?;

        Ok(())
    }

    fn tiny86_write(&self, w: &mut impl Write) -> Result<()> {
        if self.instr.len() > TINY86_MAX_INSTR_LEN {
            return Err(anyhow!(
                "invariant failure: instruction len {} > {}",
                self.instr.len(),
                TINY86_MAX_INSTR_LEN
            ));
        }

        match self.hints.len() {
            0 => {
                MemoryHint::pad_write(w)?;
                MemoryHint::pad_write(w)?;
            }
            1 => {
                self.hints[0].tiny86_write(w)?;
                MemoryHint::pad_write(w)?;
            }
            2 => {
                self.hints[0].tiny86_write(w)?;
                self.hints[1].tiny86_write(w)?;
            }
            _ => {
                return Err(anyhow!(
                    "invariant failure: more than {} hints",
                    TINY86_MAX_HINTS
                ));
            }
        }

        self.regs.tiny86_write(w)?;

        let mut instr = vec![0x90u8; TINY86_MAX_INSTR_LEN];
        instr.splice(..self.instr.len(), self.instr.iter().cloned());
        instr.reverse();

        w.write_all(&instr)?;

        Ok(())
    }
}

impl<T> Bitstring for T
where
    T: Tiny86Write,
{
    fn bitstring(&self) -> Result<String> {
        let mut buf = vec![];
        self.tiny86_write(&mut buf)?;

        // Probably not the fastest.
        Ok(buf
            .iter()
            .map(|b| format!("{:08b}", b))
            .collect::<Vec<_>>()
            .concat())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{MemoryMask, MemoryOp};

    fn dummy_word_hint() -> MemoryHint {
        MemoryHint {
            address: 0xababababcdcdcdcd,
            operation: MemoryOp::Write,
            mask: MemoryMask::Word,
            data: vec![0xcc, 0xcc],
        }
    }

    fn dummy_dword_hint() -> MemoryHint {
        MemoryHint {
            address: 0xababababcdcdcdcd,
            operation: MemoryOp::Write,
            mask: MemoryMask::DWord,
            data: vec![0x41, 0x41, 0x41, 0x41],
        }
    }

    fn dummy_regfile() -> RegisterFile {
        RegisterFile {
            rax: 0x11111111,
            rbx: 0x22222222,
            rcx: 0x33333333,
            rdx: 0x44444444,
            rsi: 0x55555555,
            rdi: 0x66666666,
            rsp: 0x77777777,
            rbp: 0x88888888,
            rip: 0x99999999,
            rflags: 0xaaaaaaaa,
            ..Default::default()
        }
    }

    fn dummy_step(num_hints: usize) -> Step {
        let hints = vec![dummy_dword_hint(); num_hints];

        Step {
            instr: vec![0xc3],
            regs: dummy_regfile(),
            hints: hints,
        }
    }

    fn dword_hint_asserts(hint_bytes: &[u8]) {
        assert_eq!(
            &hint_bytes[..MemoryHint::serialized_size()],
            vec![0b10000110, 0xcd, 0xcd, 0xcd, 0xcd, 0x41, 0x41, 0x41, 0x41]
        );
    }

    fn regfile_asserts(regfile_bytes: &[u8]) {
        // These values are consistent with dummy_regfile.
        assert_eq!(&regfile_bytes[..4], vec![0x11; 4]);
        assert_eq!(&regfile_bytes[4..8], vec![0x22; 4]);
        assert_eq!(&regfile_bytes[8..12], vec![0x33; 4]);
        assert_eq!(&regfile_bytes[12..16], vec![0x44; 4]);
        assert_eq!(&regfile_bytes[16..20], vec![0x55; 4]);
        assert_eq!(&regfile_bytes[20..24], vec![0x66; 4]);
        assert_eq!(&regfile_bytes[24..28], vec![0x77; 4]);
        assert_eq!(&regfile_bytes[28..32], vec![0x88; 4]);
        assert_eq!(&regfile_bytes[32..36], vec![0x99; 4]);
        assert_eq!(&regfile_bytes[36..40], vec![0xaa; 4]);
    }

    #[test]
    fn test_write_memoryhint() {
        {
            let hint = dummy_word_hint();
            let mut buf = vec![];

            hint.tiny86_write(&mut buf)
                .expect("tiny86 hint serialization failed");

            assert_eq!(buf.len(), MemoryHint::serialized_size());

            assert_eq!(
                buf,
                vec![0b10000101, 0xcd, 0xcd, 0xcd, 0xcd, 0xcc, 0xcc, 0x00, 0x00]
            );
        }

        {
            let hint = dummy_dword_hint();
            let mut buf = vec![];

            hint.tiny86_write(&mut buf)
                .expect("tiny86 hint serialization failed");

            assert_eq!(buf.len(), MemoryHint::serialized_size());

            dword_hint_asserts(&buf);
        }
    }

    #[test]
    fn test_write_regfile() {
        let regs = dummy_regfile();
        let mut buf = vec![];

        regs.tiny86_write(&mut buf)
            .expect("tiny86 regfile serialization failed");

        assert_eq!(buf.len(), RegisterFile::serialized_size());

        regfile_asserts(&buf);
    }

    #[test]
    fn test_write_step() {
        // No hints: succeeds, hints are zeroed
        {
            let step = dummy_step(0);
            let mut buf = vec![];

            step.tiny86_write(&mut buf)
                .expect("tiny86 step serialization failed");

            assert_eq!(buf.len(), Step::serialized_size());

            // First, two empty memory hints.
            let mut off = MemoryHint::serialized_size() * 2;
            assert_eq!(&buf[0..off], vec![0; off]);

            // Next, the register file.
            regfile_asserts(&buf[off..]);
            off += RegisterFile::serialized_size();

            // The instruction is a RET, padded out with NOPs.
            assert_eq!(*buf.last().unwrap(), 0xc3);
            assert_eq!(
                &buf[off..(off + TINY86_MAX_INSTR_LEN - 1)],
                vec![0x90; TINY86_MAX_INSTR_LEN - 1]
            );
        }

        // One hint: first hint is filled, second is zeroed
        {
            let step = dummy_step(1);
            let mut buf = vec![];

            step.tiny86_write(&mut buf)
                .expect("tiny86 step serialization failed");

            assert_eq!(buf.len(), Step::serialized_size());

            // One memory hint, followed by empty padding.
            dword_hint_asserts(&buf);
            let mut off = MemoryHint::serialized_size() * 2;
            assert_eq!(
                &buf[MemoryHint::serialized_size()..off],
                vec![0; MemoryHint::serialized_size()]
            );

            regfile_asserts(&buf[off..]);
            off += RegisterFile::serialized_size();

            // The instruction is a RET, padded out with NOPs.
            assert_eq!(*buf.last().unwrap(), 0xc3);
            assert_eq!(
                &buf[off..(off + TINY86_MAX_INSTR_LEN - 1)],
                vec![0x90; TINY86_MAX_INSTR_LEN - 1]
            );
        }

        // Two hints: both are filled
        {
            let step = dummy_step(2);
            let mut buf = vec![];

            step.tiny86_write(&mut buf)
                .expect("tiny86 step serialization failed");

            assert_eq!(buf.len(), Step::serialized_size());

            // Two valid memory hints.
            dword_hint_asserts(&buf);
            dword_hint_asserts(&buf[MemoryHint::serialized_size()..]);
            let mut off = MemoryHint::serialized_size() * 2;

            regfile_asserts(&buf[off..]);
            off += RegisterFile::serialized_size();

            // The instruction is a RET, padded out with NOPs.
            assert_eq!(*buf.last().unwrap(), 0xc3);
            assert_eq!(
                &buf[off..(off + TINY86_MAX_INSTR_LEN - 1)],
                vec![0x90; TINY86_MAX_INSTR_LEN - 1]
            );
        }

        // Three or more: error
        {
            let step = dummy_step(3);
            let mut buf = vec![];

            assert!(step.tiny86_write(&mut buf).is_err());
        }
    }
}
