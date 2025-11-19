# Clean Install Guide

Follow these steps to remove every trace of a previous CodeGraph MCP installation and rebuild from scratch.

1. **Stop running servers**  
   Close every terminal session that is running `codegraph start …` (Ctrl+C) so no process holds binaries or logs open.

2. **Remove installed binaries**  
   If you used the installer, it dropped symlinks/binaries into `/usr/local/bin`. Remove them:
   ```bash
   sudo rm -f /usr/local/bin/codegraph /usr/local/bin/codegraph-official
   ```

3. **Delete the installer support directory**  
   The installer caches logs/config under `~/.codegraph`. Remove it to avoid stale state:
   ```bash
   rm -rf ~/.codegraph
   ```

4. **Clean the repository build artifacts**  
   From the repo root run both commands to guarantee a fresh rebuild:
   ```bash
   cargo clean
   rm -rf target
   ```

5. **(Optional) Remove Python test environments**  
   If you created a virtualenv or pip install for `test_http_mcp.py`, delete that environment so you can reinstall dependencies cleanly.

6. **Re-run the installer**  
   Execute the installer once (choose the script you normally use, e.g. cloud):
   ```bash
   ./install-codegraph-cloud.sh
   ```
   This recreates `/usr/local/bin/codegraph` pointing at the freshly built release binary.

7. **Rebuild your development binary**  
   Inside the repo, rebuild with the required features so the local `target/` tree matches your latest changes:
   ```bash
   cargo build -p codegraph-mcp --features "server-http"
   ```

8. **Start the MCP server with the new binary**  
   Launch STDIO or HTTP using the freshly built binary (debug path shown here):
   ```bash
   ./target/debug/codegraph start mcp http --port 3003
   ```
   Replace `http` with `stdio` or use the release path if desired.

Once these steps are complete, you are running on a completely clean install and can rerun the MCP tests with confidence.
