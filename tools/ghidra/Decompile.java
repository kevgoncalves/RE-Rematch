// Decompile functions from an already analysed Ghidra project, for studying the game's rules.
//
//   analyzeHeadless <projectDir> <project> -process rc.exe -noanalysis -readOnly
//       -scriptPath tools/ghidra -postScript Decompile.java <outFile> <arg> ...
//
// Arguments:
//   <addr>          decompile the function containing addr
//   str:Text        find strings containing Text and list the functions that reference them
//   vt:<addr>:<n>   list n vtable slots starting at addr (function address and name)
//   refs:<addr>     list the places that reference addr
// The output is derived from the game's code: keep it local and write any rule found in it in
// your own words.
//@category RE Rematch

import java.io.File;
import java.io.PrintWriter;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.data.StringDataInstance;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.DataIterator;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;

public class Decompile extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        try (PrintWriter out = new PrintWriter(new File(args[0]), "UTF-8")) {
            DecompInterface decomp = new DecompInterface();
            decomp.openProgram(currentProgram);
            for (int i = 1; i < args.length; i++) {
                String arg = args[i];
                if (arg.startsWith("str:")) {
                    findString(out, arg.substring(4));
                    continue;
                }
                if (arg.startsWith("vt:")) {
                    String[] p = arg.split(":");
                    Address base = toAddr(p[1]);
                    for (int k = 0; k < Integer.parseInt(p[2]); k++) {
                        Address fa = toAddr(getLong(base.add(k * 8L)));
                        Function f = getFunctionAt(fa);
                        out.println("// vt[" + k + "] " + fa + " " + (f == null ? "?" : f.getName()));
                    }
                    continue;
                }
                if (arg.startsWith("refs:")) {
                    for (Reference ref : getReferencesTo(toAddr(arg.substring(5)))) {
                        Function f = getFunctionContaining(ref.getFromAddress());
                        out.println("// ref from " + ref.getFromAddress() + (f == null ? "" : " in " + f.getName() + " @ " + f.getEntryPoint()));
                    }
                    continue;
                }
                Address addr = toAddr(arg);
                Function f = getFunctionContaining(addr);
                if (f == null) {
                    out.println("// no function at " + arg);
                    continue;
                }
                out.println("// ==== " + f.getName() + " @ " + f.getEntryPoint());
                DecompileResults r = decomp.decompileFunction(f, 120, monitor);
                out.println(r.decompileCompleted() ? r.getDecompiledFunction().getC() : "// failed: " + r.getErrorMessage());
            }
            decomp.dispose();
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
