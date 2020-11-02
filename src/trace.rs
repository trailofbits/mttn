use anyhow::{anyhow, Context, Result};
use iced_x86::{
    Code, Decoder, DecoderOptions, Instruction, InstructionInfoFactory, InstructionInfoOptions,
    MemorySize, Mnemonic, OpAccess, Register,
};
use nix::sys::ptrace;
use nix::sys::signal;
use nix::sys::uio;
use nix::sys::wait;
use nix::unistd::Pid;
use rangemap::RangeMap;
use serde::Serialize;
use spawn_ptrace::CommandPtraceSpawn;

use std::convert::{TryFrom, TryInto};
use std::process::Command;

const MAX_INSTR_LEN: usize = 15;

/// Represents the width of a concrete memory operation.
///
/// All `mttn` memory operations are 1, 2, 4, or 8 bytes.
/// Larger operations are either modeled as multiple individual operations
/// (if caused by a `REP` prefix), ignored (if configured), or cause a fatal error.
#[derive(Clone, Copy, Debug, Serialize)]
pub enum MemoryMask {
    Byte = 1,
    Word = 2,
    DWord = 4,
    QWord = 8,
}

impl TryFrom<u64> for MemoryMask {
    type Error = anyhow::Error;

    fn try_from(size: u64) -> Result<Self> {
        Ok(match size {
            1 => MemoryMask::Byte,
            2 => MemoryMask::Word,
            4 => MemoryMask::DWord,
            8 => MemoryMask::QWord,
            _ => return Err(anyhow!("size {} doesn't have a supported mask", size)),
        })
    }
}

impl TryFrom<Register> for MemoryMask {
    type Error = anyhow::Error;

    fn try_from(reg: Register) -> Result<Self> {
        (reg.info().size() as u64).try_into()
    }
}

/// The access disposition of a concrete memory operation.
///
/// All `mttn` operations are either `Read` or `Write`. Instructions that
/// perform a read-and-update are modeled with two separate operations.
/// Instructions that perform conditional reads or writes are modeled only
/// if the conditional memory operation actually took place during the trace.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum MemoryOp {
    Read,
    Write,
}

/// Represents an entire traced memory operation, including its kind (`MemoryOp`),
/// size (`MemoryMask`), concrete address, and actual read or written data.
#[derive(Debug, Serialize)]
pub struct MemoryHint {
    address: u64,
    operation: MemoryOp,
    mask: MemoryMask,
    data: u64,
}

/// Represents an individual step in the trace, including the raw instruction bytes,
/// the register file state before execution, and any memory operations that result
/// from execution.
#[derive(Debug, Serialize)]
pub struct Step {
    instr: Vec<u8>,
    regs: RegisterFile,
    hints: Vec<MemoryHint>,
}

/// Represents the (usermode) register file.
///
/// Only the standard addressable registers, plus `RFLAGS`, are tracked.
/// `mttn` assumes that all segment base addresses are `0` and therefore doesn't
/// track them (with the exception of the `FSBASE` and `GSBASE` MSRs).
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct RegisterFile {
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rsp: u64,
    rbp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rip: u64,
    rflags: u64,
    fs_base: u64,
    gs_base: u64,
}

impl RegisterFile {
    /// Given a symbolic iced-x86 register, concretize it against the register file.
    /// Narrows the result, as appropriate.
    ///
    /// Untracked registers result in an `Err` result.
    fn value(&self, reg: Register) -> Result<u64> {
        match reg {
            // 8 bit regs.
            Register::AL => Ok((self.rax as u8).into()),
            Register::BL => Ok((self.rbx as u8).into()),
            Register::CL => Ok((self.rcx as u8).into()),
            Register::DL => Ok((self.rdx as u8).into()),
            Register::AH => Ok(((self.rax >> 8) as u8).into()),
            Register::BH => Ok(((self.rbx >> 8) as u8).into()),
            Register::CH => Ok(((self.rcx >> 8) as u8).into()),
            Register::DH => Ok(((self.rdx >> 8) as u8).into()),
            Register::R8L => Ok((self.r8 as u8).into()),
            Register::R9L => Ok((self.r9 as u8).into()),
            Register::R10L => Ok((self.r10 as u8).into()),
            Register::R11L => Ok((self.r11 as u8).into()),
            Register::R12L => Ok((self.r12 as u8).into()),
            Register::R13L => Ok((self.r13 as u8).into()),
            Register::R14L => Ok((self.r14 as u8).into()),
            Register::R15L => Ok((self.r15 as u8).into()),

            // 16 bit regs.
            Register::AX => Ok((self.rax as u16).into()),
            Register::BX => Ok((self.rbx as u16).into()),
            Register::CX => Ok((self.rcx as u16).into()),
            Register::DX => Ok((self.rdx as u16).into()),
            Register::SI => Ok((self.rsi as u16).into()),
            Register::DI => Ok((self.rdi as u16).into()),
            Register::SP => Ok((self.rsp as u16).into()),
            Register::BP => Ok((self.rbp as u16).into()),
            Register::R8W => Ok((self.r8 as u16).into()),
            Register::R9W => Ok((self.r9 as u16).into()),
            Register::R10W => Ok((self.r10 as u16).into()),
            Register::R11W => Ok((self.r11 as u16).into()),
            Register::R12W => Ok((self.r12 as u16).into()),
            Register::R13W => Ok((self.r13 as u16).into()),
            Register::R14W => Ok((self.r14 as u16).into()),
            Register::R15W => Ok((self.r15 as u16).into()),

            // 32 bit regs.
            Register::EAX => Ok((self.rax as u32).into()),
            Register::EBX => Ok((self.rbx as u32).into()),
            Register::ECX => Ok((self.rcx as u32).into()),
            Register::EDX => Ok((self.rdx as u32).into()),
            Register::ESI => Ok((self.rsi as u32).into()),
            Register::EDI => Ok((self.rdi as u32).into()),
            Register::ESP => Ok((self.rsp as u32).into()),
            Register::EBP => Ok((self.rbp as u32).into()),
            Register::R8D => Ok((self.r8 as u32).into()),
            Register::R9D => Ok((self.r9 as u32).into()),
            Register::R10D => Ok((self.r10 as u32).into()),
            Register::R11D => Ok((self.r11 as u32).into()),
            Register::R12D => Ok((self.r12 as u32).into()),
            Register::R13D => Ok((self.r13 as u32).into()),
            Register::R14D => Ok((self.r14 as u32).into()),
            Register::R15D => Ok((self.r15 as u32).into()),
            Register::EIP => Ok((self.rip as u32).into()),

            // 64 bit regs.
            Register::RAX => Ok(self.rax),
            Register::RBX => Ok(self.rbx),
            Register::RCX => Ok(self.rcx),
            Register::RDX => Ok(self.rdx),
            Register::RSI => Ok(self.rsi),
            Register::RDI => Ok(self.rdi),
            Register::RSP => Ok(self.rsp),
            Register::RBP => Ok(self.rbp),
            Register::R8 => Ok(self.r8),
            Register::R9 => Ok(self.r9),
            Register::R10 => Ok(self.r10),
            Register::R11 => Ok(self.r11),
            Register::R12 => Ok(self.r12),
            Register::R13 => Ok(self.r13),
            Register::R14 => Ok(self.r14),
            Register::R15 => Ok(self.r15),
            Register::RIP => Ok(self.rip),

            // FS and GS: We support these because Linux uses them for TLS.
            // We return the FSBASE and GSBASE values here, since we're returning by address
            // and not by descriptor value.
            Register::FS => Ok(self.fs_base),
            Register::GS => Ok(self.gs_base),

            // All other segment registers are treated as 0, per the Tiny86 model.
            Register::SS | Register::CS | Register::DS | Register::ES => Ok(0),

            // Everything else (vector regs, control regs, debug regs, etc) is unsupported.
            // NOTE(ww): We track rflags in this struct, but iced-x86 doesn't have a Register
            // variant for it (presumably because it's unaddressable).
            _ => Err(anyhow!("untracked register requested: {:?}", reg)),
        }
    }
}

impl From<libc::user_regs_struct> for RegisterFile {
    fn from(user_regs: libc::user_regs_struct) -> Self {
        Self {
            rax: user_regs.rax,
            rbx: user_regs.rbx,
            rcx: user_regs.rcx,
            rdx: user_regs.rdx,
            rsi: user_regs.rsi,
            rdi: user_regs.rdi,
            rsp: user_regs.rsp,
            rbp: user_regs.rbp,
            r8: user_regs.r8,
            r9: user_regs.r9,
            r10: user_regs.r10,
            r11: user_regs.r11,
            r12: user_regs.r12,
            r13: user_regs.r13,
            r14: user_regs.r14,
            r15: user_regs.r15,
            rip: user_regs.rip,
            rflags: user_regs.eflags,
            fs_base: user_regs.fs_base,
            gs_base: user_regs.gs_base,
        }
    }
}

/// Represents an actively traced program, in some indeterminate state.
///
/// Tracees are associated with their parent `Tracer`.
pub struct Tracee<'a> {
    terminated: bool,
    tracee_pid: Pid,
    tracer: &'a Tracer,
    info_factory: InstructionInfoFactory,
    register_file: RegisterFile,
    executable_pages: RangeMap<u64, Vec<u8>>,
}

impl<'a> Tracee<'a> {
    /// Create a new `Tracee` from the given PID (presumably either spawned with `PTRACE_TRACEME`
    /// or recently attached to) and `Tracer`.
    fn new(tracee_pid: Pid, tracer: &'a Tracer) -> Result<Self> {
        #[allow(clippy::redundant_field_names)]
        let mut tracee = Self {
            terminated: false,
            tracee_pid: tracee_pid,
            tracer: &tracer,
            info_factory: InstructionInfoFactory::new(),
            register_file: Default::default(),
            executable_pages: Default::default(),
        };

        // NOTE(ww): We do this after Tracee initialization to make our state
        // management just a little bit simpler.
        tracee.find_exec_pages()?;

        Ok(tracee)
    }

    /// Step the tracee forwards by one instruction, returning the trace `Step` or
    /// an `Err` if an internal tracing step fails.
    fn step(&mut self) -> Result<Step> {
        self.tracee_regs()?;
        let (instr, instr_bytes) = self.tracee_instr()?;

        // TODO(ww): Check `instr` here and perform one of two cases:
        // 1. If `instr` is an instruction that benefits from modeling/emulation
        //    (e.g. `MOVS`), then emulate it and generate its memory hints
        //    from the emulation.
        // 2. Otherwise, generate the hints as normal (do phase 1, single-step,
        //    then phase 2).

        // Hints are generated in two phases: we build a complete list of
        // expected hints (including all Read hints) in stage 1...
        let mut hints = self.tracee_hints_stage1(&instr)?;

        ptrace::step(self.tracee_pid, None)?;

        // ...then, after we've stepped the program, we fill in the data
        // associated with each Write hint in stage 2.
        self.tracee_hints_stage2(&mut hints)?;

        match wait::waitpid(self.tracee_pid, None)? {
            wait::WaitStatus::Exited(_, status) => {
                log::debug!("exited with {}", status);
                self.terminated = true;
            }
            wait::WaitStatus::Signaled(_, _, _) => {
                log::debug!("signaled");
            }
            wait::WaitStatus::Stopped(_, signal) => {
                log::debug!("stopped with {:?}", signal);
            }
            wait::WaitStatus::StillAlive => {
                log::debug!("still alive");
            }
            s => {
                log::debug!("{:?}", s);
                self.terminated = true;
            }
        }

        #[allow(clippy::redundant_field_names)]
        Ok(Step {
            instr: instr_bytes[0..instr.len()].to_vec(),
            regs: self.register_file,
            hints: hints,
        })
    }

    /// Loads the our register file from the tracee's user register state.
    fn tracee_regs(&mut self) -> Result<()> {
        self.register_file = RegisterFile::from(ptrace::getregs(self.tracee_pid)?);

        Ok(())
    }

    /// Returns the iced-x86 `Instruction` and raw instruction bytes at the tracee's
    /// current instruction pointer.
    fn tracee_instr(&self) -> Result<(Instruction, Vec<u8>)> {
        let mut bytes = vec![0u8; MAX_INSTR_LEN];
        let remote_iov = uio::RemoteIoVec {
            base: self.register_file.rip as usize,
            len: MAX_INSTR_LEN,
        };

        // TODO(ww): Check the length here.
        uio::process_vm_readv(
            self.tracee_pid,
            &[uio::IoVec::from_mut_slice(&mut bytes)],
            &[remote_iov],
        )?;

        log::debug!("fetched instruction bytes: {:?}", bytes);

        let mut decoder = Decoder::new(self.tracer.bitness, &bytes, DecoderOptions::NONE);
        decoder.set_ip(self.register_file.rip);

        let instr = decoder.decode();
        log::debug!("instr: {:?}", instr.code());

        match instr.code() {
            Code::INVALID => Err(anyhow!("invalid instruction")),
            _ => Ok((instr, bytes)),
        }
    }

    fn tracee_data_by_mask(&self, addr: u64, mask: MemoryMask) -> Result<u64> {
        let bytes = self.tracee_data(addr, mask as usize)?;

        Ok(match mask {
            MemoryMask::Byte => bytes[0] as u64,
            MemoryMask::Word => u16::from_le_bytes(bytes.as_slice().try_into()?) as u64,
            MemoryMask::DWord => u32::from_le_bytes(bytes.as_slice().try_into()?) as u64,
            MemoryMask::QWord => u64::from_le_bytes(bytes.as_slice().try_into()?) as u64,
        })
    }

    /// Reads a piece of the tracee's memory, starting at `addr`.
    fn tracee_data(&self, addr: u64, size: usize) -> Result<Vec<u8>> {
        log::debug!("attempting to read tracee @ 0x{:x} ({:?})", addr, size);

        // NOTE(ww): Could probably use ptrace::read() here since we're always <= 64 bits,
        // but I find process_vm_readv a little more readable.
        let mut bytes = vec![0u8; size];
        let remote_iov = uio::RemoteIoVec {
            base: addr as usize,
            len: size,
        };

        // NOTE(ww): A failure here indicates a bug in the tracer, not the tracee.
        // In particular it indicates that we either (1) calculated the effective
        // address incorrectly, or (2) calculated the size incorrectly.
        if let Err(e) = uio::process_vm_readv(
            self.tracee_pid,
            &[uio::IoVec::from_mut_slice(&mut bytes)],
            &[remote_iov],
        ) {
            if self.tracer.debug_on_fault {
                log::error!(
                    "Suspending the tracee ({}), detaching and exiting",
                    self.tracee_pid
                );
                ptrace::detach(self.tracee_pid, Some(signal::Signal::SIGSTOP))?;
            }

            return Err(e).with_context(|| format!("Fault: size: {:?}, address: {:x}", size, addr));
        } else {
            log::debug!("fetched data bytes: {:?}", bytes);
        }

        Ok(bytes)
    }

    fn find_exec_pages(&mut self) -> Result<()> {
        for map in rsprocmaps::from_pid(self.tracee_pid.as_raw())? {
            let map = map?;
            if !map.permissions.executable {
                continue;
            }

            let exec_range = {
                let size = map.address_range.end - map.address_range.begin;
                self.tracee_data(map.address_range.begin, size as usize)?
            };

            self.executable_pages.insert(map.address_range.begin..map.address_range.end, exec_range);
        }

        Ok(())
    }

    /// Given a string instruction (e.g., `MOVS`, `LODS`) variant, return its
    /// expected memory mask (i.e., the size in bytes that one execution
    /// reads and/or writes).
    fn mask_from_str_instr(&self, instr: &Instruction) -> Result<MemoryMask> {
        Ok(match instr.mnemonic() {
            Mnemonic::Lodsb
            | Mnemonic::Stosb
            | Mnemonic::Movsb
            | Mnemonic::Cmpsb
            | Mnemonic::Scasb => MemoryMask::Byte,
            Mnemonic::Lodsw
            | Mnemonic::Stosw
            | Mnemonic::Movsw
            | Mnemonic::Cmpsw
            | Mnemonic::Scasw => MemoryMask::Word,
            Mnemonic::Lodsd
            | Mnemonic::Stosd
            | Mnemonic::Movsd
            | Mnemonic::Cmpsd
            | Mnemonic::Scasd => MemoryMask::DWord,
            Mnemonic::Lodsq
            | Mnemonic::Stosq
            | Mnemonic::Movsq
            | Mnemonic::Cmpsq
            | Mnemonic::Scasq => MemoryMask::QWord,
            _ => return Err(anyhow!("unknown mask for instruction: {:?}", instr.code())),
        })
    }

    fn tracee_hints_stage1(&mut self, instr: &Instruction) -> Result<Vec<MemoryHint>> {
        log::debug!("memory hints stage 1");
        let mut hints = vec![];

        let info = self
            .info_factory
            .info_options(&instr, InstructionInfoOptions::NO_REGISTER_USAGE)
            .clone();

        for used_mem in info.used_memory() {
            // We model writebacks as two separate memory ops, so split them up here.
            // Also: all conditional reads and writes are in fact real reads and writes,
            // since we're single-stepping through REP'd instructions.
            let ops: &[MemoryOp] = match used_mem.access() {
                OpAccess::Read => &[MemoryOp::Read],
                OpAccess::CondRead => &[MemoryOp::Read],
                OpAccess::Write => &[MemoryOp::Write],
                OpAccess::CondWrite => &[MemoryOp::Write],
                OpAccess::ReadWrite => &[MemoryOp::Read, MemoryOp::Write],
                OpAccess::ReadCondWrite => &[MemoryOp::Read, MemoryOp::Write],
                op => return Err(anyhow!("unsupported memop: {:?}", op)),
            };

            let mask = match used_mem.memory_size() {
                MemorySize::UInt8 | MemorySize::Int8 => MemoryMask::Byte,
                MemorySize::UInt16 | MemorySize::Int16 => MemoryMask::Word,
                MemorySize::UInt32 | MemorySize::Int32 => MemoryMask::DWord,
                MemorySize::UInt64 | MemorySize::Int64 => MemoryMask::QWord,
                MemorySize::Unknown => self.mask_from_str_instr(&instr)?,
                size => {
                    if self.tracer.ignore_unsupported_memops {
                        log::warn!(
                            "unsupported memop size: {:?}: not generating a memory hint",
                            size
                        );
                        continue;
                    } else {
                        return Err(anyhow!("unsupported memsize: {:?}", size));
                    }
                }
            };

            let addr = used_mem
                .try_virtual_address(0, |reg, _, _| self.register_file.value(reg).ok())
                .ok_or_else(|| anyhow!("effective address calculation failed"))?;

            log::debug!("effective virtual addr: {:x}", addr);

            for op in ops {
                let data = match op {
                    MemoryOp::Read => self.tracee_data_by_mask(addr, mask)?,
                    MemoryOp::Write => 0,
                };

                #[allow(clippy::redundant_field_names)]
                hints.push(MemoryHint {
                    address: addr,
                    operation: *op,
                    mask: mask,
                    data: data,
                });
            }

            log::debug!("{:?}", used_mem);
        }

        Ok(hints)
    }

    fn tracee_hints_stage2(&self, hints: &mut Vec<MemoryHint>) -> Result<()> {
        log::debug!("memory hints stage 2");

        // NOTE(ww): By default, recent-ish x86 CPUs execute MOVS and STOS
        // in "fast string operation" mode. This can cause stores to not appear
        // when we expect them to, since they can be executed out-of-order.
        // The "correct" fix for this is probably to toggle the
        // fast-string-enable bit (0b) in the IA32_MISC_ENABLE MSR, but we just sleep
        // for a bit to give the CPU a chance to catch up.
        // TODO(ww): Longer term, we should just model REP'd instructions outright.
        std::thread::sleep(std::time::Duration::from_millis(1));

        for hint in hints.iter_mut() {
            if hint.operation != MemoryOp::Write {
                continue;
            }

            let data = self.tracee_data_by_mask(hint.address, hint.mask)?;
            hint.data = data;
        }

        Ok(())
    }
}

impl Iterator for Tracee<'_> {
    type Item = Result<Step>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.terminated {
            None
        } else {
            Some(self.step())
        }
    }
}

#[derive(Debug)]
pub struct Tracer {
    pub ignore_unsupported_memops: bool,
    pub debug_on_fault: bool,
    pub bitness: u32,
    pub tracee_pid: Option<Pid>,
    pub tracee_name: Option<String>,
    pub tracee_args: Vec<String>,
}

impl From<clap::ArgMatches<'_>> for Tracer {
    fn from(matches: clap::ArgMatches) -> Self {
        Self {
            ignore_unsupported_memops: matches.is_present("ignore-unsupported-memops"),
            debug_on_fault: matches.is_present("debug-on-fault"),
            bitness: matches.value_of("mode").unwrap().parse().unwrap(),
            tracee_pid: matches
                .value_of("tracee-pid")
                .map(|p| Pid::from_raw(p.parse().unwrap())),
            tracee_name: matches.value_of("tracee-name").map(Into::into),
            tracee_args: matches
                .values_of("tracee-args")
                .map(|v| v.map(|a| a.to_string()).collect())
                .unwrap_or_else(Vec::new),
        }
    }
}

impl Tracer {
    pub fn trace(&self) -> Result<Tracee> {
        let tracee_pid = if let Some(tracee_name) = &self.tracee_name {
            let child = Command::new(&tracee_name)
                .args(&self.tracee_args)
                .spawn_ptrace()?;

            log::debug!(
                "spawned {} for tracing as child {}",
                tracee_name,
                child.id()
            );

            Pid::from_raw(child.id() as i32)
        } else {
            let tracee_pid = self.tracee_pid.unwrap();
            ptrace::attach(tracee_pid)?;
            tracee_pid
        };

        // Our tracee is now live and ready to be traced, but in a stopped state.
        // We set PTRACE_O_TRACEEXIT on it to make sure it stops right before
        // finally exiting, giving us one last chance to do some inspection.
        ptrace::setoptions(tracee_pid, ptrace::Options::PTRACE_O_TRACEEXIT)?;

        Tracee::new(tracee_pid, &self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_x86::UsedMemory;

    fn dummy_regs() -> RegisterFile {
        RegisterFile {
            rax: 0x9900aabbccddeeff,
            rdi: 0x00000000feedface,
            ..Default::default()
        }
    }

    #[test]
    fn test_register_file_value() {
        let regs = dummy_regs();

        // Addressable registers always return their correctly masked values.
        assert_eq!(regs.value(Register::AL).unwrap(), 0xff);
        assert_eq!(regs.value(Register::AH).unwrap(), 0xee);
        assert_eq!(regs.value(Register::AX).unwrap(), 0xeeff);
        assert_eq!(regs.value(Register::EAX).unwrap(), 0xccddeeff);
        assert_eq!(regs.value(Register::RAX).unwrap(), 0x9900aabbccddeeff);

        // Segment registers always return a base address of 0.
        assert_eq!(regs.value(Register::AL).unwrap(), 0xff);
        assert_eq!(regs.value(Register::AH).unwrap(), 0xee);
        assert_eq!(regs.value(Register::AX).unwrap(), 0xeeff);
        assert_eq!(regs.value(Register::EAX).unwrap(), 0xccddeeff);
        assert_eq!(regs.value(Register::RAX).unwrap(), 0x9900aabbccddeeff);

        assert_eq!(regs.value(Register::SS).unwrap(), 0);
        assert_eq!(regs.value(Register::CS).unwrap(), 0);
        assert_eq!(regs.value(Register::DS).unwrap(), 0);
        assert_eq!(regs.value(Register::ES).unwrap(), 0);
        assert_eq!(regs.value(Register::FS).unwrap(), 0);
        assert_eq!(regs.value(Register::GS).unwrap(), 0);

        // Unaddressable and unsupported registers return an Err.
        assert!(regs.value(Register::ST0).is_err());
    }
}
