use anyhow::{anyhow, Result};
use iced_x86::{
    Code, Decoder, DecoderOptions, Instruction, InstructionInfoFactory, InstructionInfoOptions,
    MemorySize, OpAccess, Register, UsedMemory,
};
use nix::sys::ptrace;
use nix::sys::uio;
use nix::sys::wait;
use nix::unistd::Pid;
use num::traits::{AsPrimitive, WrappingAdd, WrappingMul};
use serde::Serialize;
use spawn_ptrace::CommandPtraceSpawn;

use std::convert::TryInto;
use std::process::Command;

const MAX_INSTR_LEN: usize = 15;

#[derive(Clone, Copy, Debug, Serialize)]
pub enum MemoryMask {
    Byte = 1,
    Word = 2,
    DWord = 4,
    QWord = 8,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum MemoryOp {
    Read,
    Write,
}

#[derive(Debug, Serialize)]
pub struct MemoryHint {
    address: u64,
    operation: MemoryOp,
    mask: MemoryMask,
    data: u64,
}

#[derive(Debug, Serialize)]
pub struct Trace {
    instr: Vec<u8>,
    regs: RegisterFile,
    hints: Vec<MemoryHint>,
}

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
}

impl RegisterFile {
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
            Register::RSI => Ok(self.rsp),
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

            // Everything else (vector regs, control regs, debug regs, etc) is unsupported.
            // NOTE(ww): We track rflags in this struct, but iced-x86 doesn't have a Register
            // variant for it (presumably because it's unaddressable).
            _ => Err(anyhow!("untracked register requested: {:?}", reg)),
        }
    }

    fn effective_address<T>(&self, mem: &UsedMemory) -> Result<u64>
    where
        T: Copy + WrappingAdd + WrappingMul + Into<u64> + 'static,
        u32: AsPrimitive<T>,
        u64: AsPrimitive<T>,
    {
        let base = match mem.base() {
            Register::None => 0u64.as_(),
            reg => self.value(reg)?.as_(),
        };

        let index = match mem.index() {
            Register::None => 0u64.as_(),
            reg => self.value(reg)?.as_(),
        };

        let effective = base
            .wrapping_add(&index.wrapping_mul(&mem.scale().as_()))
            .wrapping_add(&mem.displacement().as_());

        Ok(effective.into())
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
        }
    }
}

#[derive(Debug)]
pub struct Tracer {
    pub bitness: u32,
    pub tracee: String,
    pub tracee_args: Vec<String>,
    pub register_file: RegisterFile,
}

impl From<clap::ArgMatches<'_>> for Tracer {
    fn from(matches: clap::ArgMatches) -> Self {
        Self {
            bitness: matches.value_of("mode").unwrap().parse().unwrap(),
            tracee: matches.value_of("tracee").unwrap().into(),
            tracee_args: matches
                .values_of("tracee-args")
                .and_then(|v| Some(v.map(|a| a.to_string()).collect()))
                .unwrap_or_else(|| vec![]),
            register_file: Default::default(),
        }
    }
}

impl Tracer {
    pub fn trace(&mut self) -> Result<Vec<Trace>> {
        let tracee_pid = {
            let child = Command::new(&self.tracee)
                .args(&self.tracee_args)
                .spawn_ptrace()?;
            Pid::from_raw(child.id() as i32)
        };

        log::debug!(
            "spawned {} for tracing as child {}",
            self.tracee,
            tracee_pid
        );

        // Our tracee is now live and ready to be traced, but in a stopped state.
        // We set PTRACE_O_TRACEEXIT on it to make sure it stops right before
        // finally exiting, giving us one last chance to do some inspection.
        ptrace::setoptions(tracee_pid, ptrace::Options::PTRACE_O_TRACEEXIT)?;

        // Time to start the show.
        let mut traces = vec![];
        loop {
            self.tracee_regs(tracee_pid)?;
            let (instr, instr_bytes) = self.tracee_instr(tracee_pid)?;

            // Hints are generated in two phases: we build a complete list of
            // expected hints (including all Read hints) in stage 1...
            let mut hints = self.tracee_hints_stage1(tracee_pid, &instr)?;

            log::debug!("step!");
            ptrace::step(tracee_pid, None)?;

            // ...then, after we've stepped the program, we fill in the data
            // associated with each Write hint in stage 2.
            self.tracee_hints_stage2(tracee_pid, &mut hints)?;

            match wait::waitpid(tracee_pid, None)? {
                wait::WaitStatus::Exited(_, status) => {
                    log::debug!("exited with {}", status);
                    break;
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
                    break;
                }
            }

            #[allow(clippy::redundant_field_names)]
            traces.push(Trace {
                instr: instr_bytes[0..instr.len()].to_vec(),
                regs: self.register_file,
                hints: hints,
            })
        }

        Ok(traces)
    }

    fn tracee_regs(&mut self, pid: Pid) -> Result<()> {
        self.register_file = RegisterFile::from(ptrace::getregs(pid)?);

        Ok(())
    }

    fn tracee_instr(&self, pid: Pid) -> Result<(Instruction, Vec<u8>)> {
        let mut bytes = vec![0u8; MAX_INSTR_LEN];
        let remote_iov = uio::RemoteIoVec {
            base: self.register_file.rip as usize,
            len: MAX_INSTR_LEN,
        };

        // TODO(ww): Check the length here.
        uio::process_vm_readv(
            pid,
            &[uio::IoVec::from_mut_slice(&mut bytes)],
            &[remote_iov],
        )?;

        log::debug!("fetched instruction bytes: {:?}", bytes);

        let mut decoder = Decoder::new(self.bitness, &bytes, DecoderOptions::NONE);
        decoder.set_ip(self.register_file.rip);

        let instr = decoder.decode();
        log::debug!("instr: {:?}", instr.code());

        match instr.code() {
            Code::INVALID => Err(anyhow!("invalid instruction")),
            _ => Ok((instr, bytes)),
        }
    }

    fn tracee_data(&self, pid: Pid, addr: u64, mask: MemoryMask) -> Result<u64> {
        log::debug!("attempting to read tracee @ 0x{:x} ({:?})", addr, mask);

        // NOTE(ww): Could probably use ptrace::read() here since we're always <= 64 bits,
        // but I find process_vm_readv a little more readable.
        let mut bytes = vec![0u8; mask as usize];
        let remote_iov = uio::RemoteIoVec {
            base: addr as usize,
            len: mask as usize,
        };

        uio::process_vm_readv(
            pid,
            &[uio::IoVec::from_mut_slice(&mut bytes)],
            &[remote_iov],
        )?;

        log::debug!("fetched data bytes: {:?}", bytes);

        Ok(match mask {
            MemoryMask::Byte => bytes[0] as u64,
            MemoryMask::Word => u16::from_le_bytes(bytes.as_slice().try_into()?) as u64,
            MemoryMask::DWord => u32::from_le_bytes(bytes.as_slice().try_into()?) as u64,
            MemoryMask::QWord => u64::from_le_bytes(bytes.as_slice().try_into()?) as u64,
        })
    }

    fn tracee_hints_stage1(&self, pid: Pid, instr: &Instruction) -> Result<Vec<MemoryHint>> {
        log::debug!("memory hints stage 1");
        let mut hints = vec![];

        // TODO(ww): Memory waste.
        let info = {
            let mut info_factory = InstructionInfoFactory::new();
            info_factory
                .info_options(&instr, InstructionInfoOptions::NO_REGISTER_USAGE)
                .clone()
        };

        for used_mem in info.used_memory() {
            // We model writebacks as two separate memory ops, so split them up here.
            let ops: &[MemoryOp] = match used_mem.access() {
                OpAccess::Read => &[MemoryOp::Read],
                OpAccess::Write => &[MemoryOp::Write],
                OpAccess::ReadWrite => &[MemoryOp::Read, MemoryOp::Write],
                op => return Err(anyhow!("unsupported memop: {:?}", op)),
            };

            let mask = match used_mem.memory_size() {
                MemorySize::UInt8 | MemorySize::Int8 => MemoryMask::Byte,
                MemorySize::UInt16 | MemorySize::Int16 => MemoryMask::Word,
                MemorySize::UInt32 | MemorySize::Int32 => MemoryMask::DWord,
                MemorySize::UInt64 | MemorySize::Int64 => MemoryMask::QWord,
                size => return Err(anyhow!("unsupported memsize: {:?}", size)),
            };

            let addr = match self.bitness {
                32 => self.register_file.effective_address::<u32>(&used_mem)?,
                64 => self.register_file.effective_address::<u64>(&used_mem)?,
                _ => unreachable!(),
            };

            log::debug!("effective virtual addr: {:x}", addr);

            for op in ops {
                let data = match op {
                    MemoryOp::Read => self.tracee_data(pid, addr, mask)?,
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

    fn tracee_hints_stage2(&self, pid: Pid, hints: &mut Vec<MemoryHint>) -> Result<()> {
        log::debug!("memory hints stage 2");

        for hint in hints.iter_mut() {
            if hint.operation != MemoryOp::Write {
                continue;
            }

            let data = self.tracee_data(pid, hint.address, hint.mask)?;
            hint.data = data;
        }

        Ok(())
    }
}
