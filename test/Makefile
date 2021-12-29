MTTN := cargo run --quiet --bin mttn --

ASM_TESTS := \
	memops \
	stosb \
	stosw \
	stosd \
	alu_adc \
	alu_add_neg \
	alu_add \
	mov_r_r \
	xchg_r_r \
	lea \
	rcl \
	rol \
	jmp

C_TESTS := \
	seteip \
	smallcall


ASM_SOURCES := $(ASM_TESTS:=.s)
ASM_OBJS := $(ASM_SOURCES:.s=.o)
ASM_ELFS := $(ASM_OBJS:.o=.elf)

C_SOURCES := $(C_TESTS:=.c)
C_ELFS := $(C_SOURCES:.c=.elf)

ALL_OBJS := $(ASM_OBJS)
ALL_ELFS := $(ASM_ELFS) $(C_ELFS)

# NOTE(ww): No default traces for the C tests, since some are interactive/take
# environmental inputs.
TRACE_JSONLS := $(ASM_ELFS:.elf=.trace.jsonl)
TRACE_TEXTS := $(ASM_ELFS:.elf=.trace.txt)

all: $(ALL_ELFS)

$(ASM_ELFS): $(ASM_OBJS)
	ld -m elf_i386 $(basename $@).o -o $(basename $@).elf
	chmod +x $(basename $@).elf

$(ASM_OBJS): $(ASM_SOURCES)
	nasm -f elf32 $(basename $@).s -o $(basename $@).o

$(C_ELFS): $(C_SOURCES)
	$(CC) -fno-pic -g -m32 -mtune=i386 -Wl,-emain -nostdlib -static -std=c99 \
		$(basename $@).c -o $(basename $@).elf -mpreferred-stack-boundary=2 \
		-fno-stack-protector -z execstack

.PHONY: trace-jsonls
trace-jsonls: $(TRACE_JSONLS)

.PHONY: trace-texts
trace-texts: $(TRACE_TEXTS)

%.trace.jsonl: %.elf
	$(MTTN) -t -m32 -F jsonl ./$< > $@

%.trace.txt: %.elf
	$(MTTN) -t -m32 -F tiny86-text ./$< > $@

.PHONY: clean
clean:
	rm -rf $(ALL_OBJS) $(ALL_ELFS) $(TRACE_JSONLS) $(TRACE_TEXTS)
