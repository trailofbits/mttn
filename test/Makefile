ASM_TESTS := \
	memops \
	stosb \
	stosw \
	stosd \
	alu_adc \
	alu_add_neg \
	alu_add

C_TESTS := \
	seteip


ASM_SOURCES := $(ASM_TESTS:=.s)
ASM_OBJS := $(ASM_SOURCES:.s=.o)
ASM_ELFS := $(ASM_OBJS:.o=.elf)

C_SOURCES := $(C_TESTS:=.c)
C_ELFS := $(C_SOURCES:.c=.elf)

ALL_OBJS := $(ASM_OBJS)
ALL_ELFS := $(ASM_ELFS) $(C_ELFS)

all: $(ALL_ELFS)

$(ASM_ELFS): $(ASM_OBJS)
	ld -m elf_i386 $(basename $@).o -o $(basename $@).elf
	chmod +x $(basename $@).elf

$(ASM_OBJS): $(ASM_SOURCES)
	nasm -f elf32 $(basename $@).s -o $(basename $@).o

$(C_ELFS): $(C_SOURCES)
	$(CC) -g -m32 -mtune=i386 -Wl,-emain -nostdlib -static -std=c99 $(basename $@).c -o $(basename $@).elf -mpreferred-stack-boundary=2 -fno-stack-protector -z execstack

.PHONY: clean
clean:
	rm -rf $(ALL_OBJS) $(ALL_ELFS)
