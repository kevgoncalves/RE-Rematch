// Names the game's reflected functions (from tools/natives.py), then decompiles every function in
// the given address ranges, for studying the game's rules.
//
//   analyzeHeadless <projectDir> <project> -process rc.exe -noanalysis
//       -scriptPath tools/ghidra -postScript NameAndDump.java <natives.tsv> <outDir> <lo-hi> ...
//
// Each UFUNCTION's exec thunk gets "Class__execName"; when the thunk calls exactly one function
// inside the ranges, that callee gets "Class__Name" (the C++ implementation). Output: one file
// per 1000 functions in outDir. It is derived from the game's code: keep it local and write any
// rule found in it in your own words.
//@category RE Rematch

import java.io.BufferedReader;
import java.io.File;
import java.io.FileReader;
import java.io.PrintWriter;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileOptions;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionIterator;
import ghidra.program.model.symbol.SourceType;

public class NameAndDump extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        AddressSet ranges = new AddressSet();
        for (int i = 2; i < args.length; i++) {
            String[] lh = args[i].split("-");
            ranges.add(toAddr(lh[0]), toAddr(lh[1]));
        }

        int named = 0, impls = 0;
        try (BufferedReader in = new BufferedReader(new FileReader(args[0]))) {
            String line;
            while ((line = in.readLine()) != null) {
                String[] cols = line.split("\t");
                Address addr = toAddr(cols[0]);
                Function f = getFunctionAt(addr);
                if (f == null) {
                    f = createFunction(addr, null);
                }
                if (f == null || f.getSymbol().getSource() == SourceType.USER_DEFINED) {
                    continue; // folded with another thunk, keep the first name
                }
                String name = cols[1].replace("::", "__");
                f.setName(name, SourceType.USER_DEFINED);
                named++;
                List<Function> inside = new ArrayList<>();
                Set<Function> callees = f.getCalledFunctions(monitor);
                for (Function c : callees) {
                    if (ranges.contains(c.getEntryPoint()) && c.getSymbol().getSource() != SourceType.USER_DEFINED) {
                        inside.add(c);
                    }
                }
                if (inside.size() == 1 && name.contains("__exec")) {
                    inside.get(0).setName(name.replace("__exec", "__"), SourceType.USER_DEFINED);
                    impls++;
                }
            }
        }
        println("named " + named + " thunks and " + impls + " implementations");

        DecompInterface decomp = new DecompInterface();
        decomp.setOptions(new DecompileOptions());
        decomp.openProgram(currentProgram);
        File outDir = new File(args[1]);
        outDir.mkdirs();
        FunctionIterator it = currentProgram.getFunctionManager().getFunctions(ranges, true);
        int n = 0;
        PrintWriter out = null;
        while (it.hasNext() && !monitor.isCancelled()) {
            Function f = it.next();
            if (n % 1000 == 0) {
                if (out != null) {
                    out.close();
                }
                out = new PrintWriter(new File(outDir, String.format("part_%03d.c", n / 1000)), "UTF-8");
                println("decompiled " + n);
            }
            out.println("// ==== " + f.getName() + " @ " + f.getEntryPoint());
            DecompileResults r = decomp.decompileFunction(f, 60, monitor);
            out.println(r.decompileCompleted() ? r.getDecompiledFunction().getC() : "// failed: " + r.getErrorMessage());
            n++;
        }
        if (out != null) {
            out.close();
        }
        decomp.dispose();
        println("decompiled " + n + " functions");
    }
}
