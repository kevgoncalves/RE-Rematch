// Keep the analysed project open and answer requests from files, so each query skips the
// minutes-long project load. Requests: <dir>/req.txt (one argument per line, as Decompile.java,
// plus "quit"); the answer is written to <dir>/resp.txt and req.txt is removed.
//
//   analyzeHeadless <projectDir> <project> -process rc.exe -noanalysis -readOnly
//       -scriptPath tools/ghidra -postScript Serve.java <dir>
//
// The output is derived from the game's code: keep it local and write any rule found in it in
// your own words.
//@category RE Rematch

import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.List;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.data.StringDataInstance;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.DataIterator;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;

public class Serve extends GhidraScript {
    private DecompInterface decomp;

    @Override
    public void run() throws Exception {
        File dir = new File(getScriptArgs()[0]);
        File req = new File(dir, "req.txt");
        File resp = new File(dir, "resp.txt");
        decomp = new DecompInterface();
        decomp.openProgram(currentProgram);
        Files.writeString(new File(dir, "ready.txt").toPath(), "ready");
        while (true) {
            if (!req.exists()) {
                Thread.sleep(200);
                continue;
            }
            Thread.sleep(100);
            List<String> args = Files.readAllLines(req.toPath(), StandardCharsets.UTF_8);
            StringWriter buf = new StringWriter();
            PrintWriter out = new PrintWriter(buf);
            boolean quit = false;
            for (String arg : args) {
                arg = arg.trim();
                if (arg.isEmpty()) {
                    continue;
                }
                if (arg.equals("quit")) {
                    quit = true;
                    break;
                }
                try {
                    handle(out, arg);
                } catch (Exception e) {
                    out.println("// error on " + arg + ": " + e);
                }
            }
            out.flush();
            File tmp = new File(dir, "resp.tmp");
            Files.writeString(tmp.toPath(), buf.toString(), StandardCharsets.UTF_8);
            req.delete();
            tmp.renameTo(resp);
            if (quit) {
                break;
            }
        }
        decomp.dispose();
    }

    private void handle(PrintWriter out, String arg) throws Exception {
        if (arg.startsWith("str:")) {
            findString(out, arg.substring(4));
        } else if (arg.startsWith("vt:")) {
            String[] p = arg.split(":");
            Address base = toAddr(p[1]);
            for (int k = 0; k < Integer.parseInt(p[2]); k++) {
                Address fa = toAddr(getLong(base.add(k * 8L)));
                Function f = getFunctionAt(fa);
                out.println("// vt[" + k + "] " + fa + " " + (f == null ? "?" : f.getName()));
            }
        } else if (arg.startsWith("scan:")) {
            // scan:<lo>-<hi>:pat|pat - functions in [lo, hi) whose instructions contain every
            // pattern (instruction text, e.g. "+ 0x2d0]").
            String[] parts = arg.substring(5).split(":", 2);
            String[] range = parts[0].split("-");
            Address lo = toAddr(range[0]);
            Address hi = toAddr(range[1]);
            String[] pats = parts[1].split("\\|");
            int hits = 0;
            for (Function f : currentProgram.getFunctionManager().getFunctions(lo, true)) {
                if (f.getEntryPoint().compareTo(hi) >= 0) {
                    break;
                }
                boolean[] seen = new boolean[pats.length];
                int left = pats.length;
                for (ghidra.program.model.listing.Instruction ins : currentProgram.getListing().getInstructions(f.getBody(), true)) {
                    String s = ins.toString();
                    for (int k = 0; k < pats.length; k++) {
                        if (!seen[k] && s.contains(pats[k])) {
                            seen[k] = true;
                            left--;
                        }
                    }
                    if (left == 0) {
                        break;
                    }
                }
                if (left == 0) {
                    out.println("// match " + f.getName() + " @ " + f.getEntryPoint() + " size " + f.getBody().getNumAddresses());
                    if (++hits >= 100) {
                        break;
                    }
                }
            }
        } else if (arg.startsWith("asm:")) {
            // asm:<addr>:<n> - n instructions from addr
            String[] p = arg.split(":");
            ghidra.program.model.listing.Instruction ins = getInstructionAt(toAddr(p[1]));
            for (int k = 0; ins != null && k < Integer.parseInt(p[2]); k++, ins = ins.getNext()) {
                out.println(ins.getAddress() + "  " + ins);
            }
        } else if (arg.startsWith("refs:")) {
            for (Reference ref : getReferencesTo(toAddr(arg.substring(5)))) {
                Function f = getFunctionContaining(ref.getFromAddress());
                out.println("// ref from " + ref.getFromAddress() + (f == null ? "" : " in " + f.getName() + " @ " + f.getEntryPoint()));
            }
        } else {
            Address addr = toAddr(arg);
            Function f = getFunctionContaining(addr);
            if (f == null) {
                disassemble(addr);
                f = createFunction(addr, null);
            }
            if (f == null) {
                out.println("// no function at " + arg);
                return;
            }
            out.println("// ==== " + f.getName() + " @ " + f.getEntryPoint());
            DecompileResults r = decomp.decompileFunction(f, 120, monitor);
            out.println(r.decompileCompleted() ? r.getDecompiledFunction().getC() : "// failed: " + r.getErrorMessage());
        }
    }

    private void findString(PrintWriter out, String text) {
        DataIterator it = currentProgram.getListing().getDefinedData(true);
        int found = 0;
        while (it.hasNext() && found < 50) {
            Data d = it.next();
            StringDataInstance s = StringDataInstance.getStringDataInstance(d);
            if (s == StringDataInstance.NULL_INSTANCE) {
                continue;
            }
            String value = s.getStringValue();
            if (value == null || !value.contains(text)) {
                continue;
            }
            found++;
            out.println("// string \"" + value + "\" @ " + d.getAddress());
            for (Reference ref : getReferencesTo(d.getAddress())) {
                Function f = getFunctionContaining(ref.getFromAddress());
                out.println("//   ref from " + ref.getFromAddress() + (f == null ? "" : " in " + f.getName() + " @ " + f.getEntryPoint()));
            }
        }
        if (found == 0) {
            out.println("// no defined string contains \"" + text + "\"");
        }
    }
}
