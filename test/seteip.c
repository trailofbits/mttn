/* seteip.c: set EIP from the command line
 *
 * cc -g -m32 -static -std=c99 seteip.c -o seteip.elf -mpreferred-stack-boundary=2 -fno-stack-protector -z execstack
 */

#include <string.h>

int main(int argc, char const *argv[]) {
  char lol[8];

  // saved EIP is 16 bytes (thanks to padding) from ESP, so we write directly
  // here instead of expecting padding in the input
  strcpy(lol + 16, argv[1]);
  return 0;
}
