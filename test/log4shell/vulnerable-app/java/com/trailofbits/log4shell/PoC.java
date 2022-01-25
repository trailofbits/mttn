package com.trailofbits.log4shell;

import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

import java.io.IOException;

class PoC {
    public static void main(String[] args) {
        Logger logger = LogManager.getLogger(PoC.class);
        boolean firstArg = true;
        for(String arg : args) {
            if(firstArg && arg.equals("--break-on-start")) {
                long pid = ProcessHandle.current().pid();
                System.err.print("PID: ");
                System.err.flush();
                System.out.print(pid);
                System.err.println("");
                System.err.flush();
//                System.err.println("Press any key to continue...");
                try {
                    Runtime.getRuntime().exec("kill -SIGSTOP " + pid);
                    Thread.sleep(5000);
                } catch(IOException e) {
                    System.err.println("Error sending SIGSTOP to PID " + pid);
                } catch (InterruptedException e) {
                    System.err.println("Interrupted while calling sleep()");
                }
//                byte[] buffer = new byte[1];
//                try {
//                   System.in.read(buffer);
//                } catch (IOException e) {
//                   System.err.println("Error reading from STDIN");
//                   System.exit(1);
//                }
            } else {
                System.err.println("Logging \"" + arg + "\"");
                logger.error(arg);
            }
            firstArg = false;
        }
    }
}
