"""Python 3.11+: python3 tests/worktrees.py /path/to/openidea-server"""

import asyncio
import base64
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from zipfile import ZipFile

PLUGIN = Path(__file__).resolve().parents[1]
ID = "dengliu.gitlink"


def package(tmp):
    tool = Path(sys.argv[1]).with_name("openidea-plugin")
    key, archive = tmp / "test-key", tmp / "gitlink.oiplugin"
    subprocess.run([tool, "keygen", key], check=True, stdout=subprocess.DEVNULL)
    signing = ["--key", key, "--publisher", "dengliu"]
    subprocess.run([tool, "pack", PLUGIN, *signing, "--out", archive], check=True)
    subprocess.run([tool, "index", tmp, *signing], check=True)
    entry, = json.loads((tmp / "index.json").read_text())["plugins"]
    assert entry["id"] == ID
    with ZipFile(archive) as z:
        files = [{"path": p, "data": base64.b64encode(z.read(p)).decode()} for p in z.namelist()
                 if p not in {"README.md", "LICENSE", "NOTICE", "SIGNATURE.json"}]
    grant = ["vcs.read", "editor.read", "project.read", "clipboard"]
    return {"packages": [{"id": ID, "digest": entry["digest"], "granted": grant}], "files": {ID: files}}


def git(path, *args):
    env = {**os.environ, "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"}
    return subprocess.check_output(["git", "-C", path, *args], env=env, text=True).strip()


def repository(path):
    path.mkdir(parents=True)
    git(path, "init", "-q", "-b", path.name)
    (path / "notes.txt").write_text(f"{path.name}\nsecond line\n")
    git(path, "add", "notes.txt")
    git(path, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
        "-c", "commit.gpgsign=false", "commit", "-qm", path.name)
    git(path, "remote", "add", "origin", f"https://github.com/example/{path.name}.git")
    return git(path, "rev-parse", "HEAD")


async def exercise(server, tmp):
    workspace = tmp / "workspace"
    alpha, beta = workspace / "alpha", workspace / "beta"
    nested = alpha / "nested"
    commits = {repo: repository(repo) for repo in (alpha, beta)}
    source = tmp / "source" / "nested"
    commits[nested] = repository(source)
    git(alpha, "-c", "protocol.file.allow=always", "submodule", "add", "-q", str(source), "nested")
    git(nested, "remote", "set-url", "origin", "https://github.com/example/nested.git")
    git(nested, "checkout", "-q", "--detach")
    loose = workspace / "outside.txt"
    loose.touch()
    events = []
    request_id = 0

    def send(method, params):
        nonlocal request_id
        request_id += 1
        data = json.dumps({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params}).encode()
        server.stdin.write(f"Content-Length: {len(data)}\r\n\r\n".encode() + data)
        return request_id

    async def call(method, params, answer=...):
        expected = send(method, params)
        while True:
            headers = await server.stdout.readuntil(b"\r\n\r\n")
            size = next(int(line.split(b":")[1]) for line in headers.lower().splitlines()
                        if line.startswith(b"content-length:"))
            msg = json.loads(await server.stdout.readexactly(size))
            if msg.get("method") == "plugin":
                event = msg["params"]["event"]
                events.append(event)
                if event["type"] == "promptRequested":
                    assert answer is not ..., "unexpected repository picker"
                    assert {item["key"] for item in event["prompt"]["items"]} == {str(r) for r in commits}
                    send("invoke", {"window": "main", "cmd": "plugin_answer_prompt",
                                    "args": {"promptId": event["promptId"], "answer": answer}})
            elif msg.get("id") == expected:
                if "error" in msg:
                    raise RuntimeError(msg["error"]["message"])
                return msg["result"]

    def invoke(cmd, args, answer=...):
        return call("invoke", {"window": "main", "cmd": cmd, "args": args}, answer)

    def run(command, answer=..., **args):
        events.clear()
        return invoke("plugin_run_command", {"pluginId": ID, "commandId": command, **args}, answer)

    async def open_project(root, count):
        await invoke("open_project", {"path": str(root)})
        await invoke("project_trust_set", {"path": str(root), "trusted": True, "folder": False})
        async with asyncio.timeout(15):
            while len(await invoke("vcs_roots", {})) != count:
                await asyncio.sleep(0.05)

    await call("initialize", {"version": 2})
    await open_project(workspace, 3)
    report = await invoke("plugin_sync_packages", package(tmp))
    assert report["installed"] == [ID], report
    editor = {"path": str(alpha / "notes.txt"), "language": "text", "selections": [
        {"start": {"line": 0, "col": 0}, "end": {"line": 1, "col": 4}}]}
    base = "https://github.com/example"
    markdown = f"[notes.txt:1-2]({base}/alpha/blob/alpha/notes.txt#L1-L2)"
    assert await run("copy-markdown", editor=editor) == markdown
    assert any(e.get("text") == markdown and e["type"] == "copyToClipboard" for e in events)
    url = f"{base}/beta/blob/beta/notes.txt"
    assert await run("open", arg=str(beta / "notes.txt"), editor=editor) == url
    assert any(e.get("url") == url and e["type"] == "openUrl" for e in events)
    sha = commits[nested]
    assert await run("copy", paths=[str(nested / "notes.txt")], editor=editor) == f"{base}/nested/blob/{sha}/notes.txt"
    assert await run("copy", arg=str(nested)) == f"{base}/nested/tree/{sha}"
    assert await run("copy-commit", paths=[str(nested)], editor=editor) == f"{base}/nested/commit/{commits[nested]}"
    assert await run("open-commit", arg=sha, paths=[str(nested)]) == f"{base}/nested/commit/{sha}"
    assert await run("copy-commit-markdown", arg=sha, paths=[str(nested)]) == f"[{sha[:8]}]({base}/nested/commit/{sha})"
    assert await run("copy-commit", answer=[str(beta)]) == f"{base}/beta/commit/{commits[beta]}"
    assert await run("copy", answer=None) is None
    assert not any(e["type"] in ("copyToClipboard", "openUrl") for e in events)
    try:
        await run("copy", arg=str(loose), editor=editor)
    except RuntimeError as exc:
        assert "not in a Git repository" in str(exc), exc
    else:
        raise AssertionError("file outside Git silently got a link")
    await open_project(beta, 1)
    assert await run("copy-commit") == f"{base}/beta/commit/{commits[beta]}"
    print("PASS: editor selection, explicit path, clipboard/open events, nested file and directory,"
          " commit, picker, cancel, outside Git and single repository")


async def main():
    with tempfile.TemporaryDirectory(prefix="gitlink-e2e-") as tmp:
        root = Path(tmp).resolve()
        server = await asyncio.create_subprocess_exec(sys.argv[1], "--stdio",
            stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE,
            env={**os.environ, "OPENIDEA_SERVER_HOME": str(root / "server"), "OPENIDEA_LOG": "warn"})
        try:
            async with asyncio.timeout(90):
                await exercise(server, root)
        finally:
            server.stdin.close()
            try:
                await asyncio.wait_for(server.wait(), 15)
            except TimeoutError:
                server.kill()
                await server.wait()


if __name__ == "__main__":
    asyncio.run(main())
