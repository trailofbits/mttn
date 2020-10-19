TESTS := \
	memops \
	stosb \
	stosw \
	stosd

ASMS := $(TESTS:=.s)
OBJS := $(TESTS:=.o)
ELFS := $(TESTS:=.elf)

all: $(ELFS)

$(ELFS): $(OBJS)
	ld -m elf_i386 $(basename $@).o -o $(basename $@).elf
	chmod +x $(basename $@).elf

$(OBJS): $(ASMS)
	nasm -f elf32 $(basename $@).s -o $(basename $@).o

.PHONY: clean
clean:
	rm -rf $(OBJS) $(ELFS)
