"""Claude Code セッションデータのパーサーモジュール。

~/.claude/projects/ 以下のセッションデータ（JSONL形式）を読み込み、パースする。
"""

import json
import os
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Optional


CLAUDE_PROJECTS_DIR = Path.home() / ".claude" / "projects"


@dataclass
class Message:
    """パース済みのメッセージ。"""

    role: str  # "user", "assistant", "system", "tool_use", "tool_result", "progress"
    text: str
    timestamp: Optional[datetime]
    tool_name: Optional[str] = None  # tool_use の場合のツール名
    tool_input: Optional[dict] = None  # tool_use の場合の入力

    @property
    def timestamp_str(self) -> str:
        if self.timestamp is None:
            return ""
        return self.timestamp.strftime("%Y-%m-%d %H:%M:%S")


@dataclass
class SessionInfo:
    """セッションのメタ情報。"""

    session_id: str
    project_name: str
    preview: str  # 最初のユーザーメッセージ（プレビュー用）
    timestamp: Optional[datetime] = None
    message_count: int = 0
    git_branch: str = ""
    summary: str = ""

    @property
    def timestamp_str(self) -> str:
        if self.timestamp is None:
            return ""
        return self.timestamp.strftime("%Y-%m-%d %H:%M:%S")


@dataclass
class ProjectInfo:
    """プロジェクトのメタ情報。"""

    dir_name: str  # ディレクトリ名（エンコード済み）
    original_path: str  # 復元されたパス
    session_count: int = 0


def _decode_project_path(dir_name: str) -> str:
    """プロジェクトディレクトリ名から元のパスを復元する。

    例: "-Users-doskoi64-src-github-com-foo" -> "/Users/doskoi64/src/github.com/foo"

    エンコード規則は単純に全ての "/" と "." を "-" に変換するもの。
    完全に正確な復元は不可能なため、既知のドメインパターンを置換するヒューリスティックを使用。
    sessions-index.json がある場合はそちらが優先される。
    """
    if not dir_name:
        return dir_name

    # エンコード済み文字列（先頭の "-" を除去）
    encoded = dir_name.lstrip("-")

    # 既知のドメインパターンを一時的なプレースホルダに置換（長いものから順に）
    # "-" を "/" に変換する前に、ドメイン内の "-" を "." に変換する必要がある
    domain_replacements = [
        ("-tech-pepabo-com-", "/tech.pepabo.com/"),
        ("-git-pepabo-com-", "/git.pepabo.com/"),
        ("-github-com-", "/github.com/"),
        ("-gitlab-com-", "/gitlab.com/"),
        ("-bitbucket-org-", "/bitbucket.org/"),
    ]
    for old, new in domain_replacements:
        encoded = encoded.replace(old, new)

    # 末尾がドメインで終わるケース
    domain_endings = [
        ("-tech-pepabo-com", "/tech.pepabo.com"),
        ("-git-pepabo-com", "/git.pepabo.com"),
        ("-github-com", "/github.com"),
        ("-gitlab-com", "/gitlab.com"),
        ("-bitbucket-org", "/bitbucket.org"),
    ]
    for old, new in domain_endings:
        if encoded.endswith(old):
            encoded = encoded[: -len(old)] + new

    # 残りの "-" を "/" に変換
    path = "/" + encoded.replace("-", "/")

    return path


def _try_get_original_path(project_dir: Path) -> Optional[str]:
    """sessions-index.json からoriginalPathまたはprojectPathを取得する。"""
    index_path = project_dir / "sessions-index.json"
    if not index_path.exists():
        return None
    try:
        with open(index_path, encoding="utf-8") as f:
            data = json.load(f)
        # originalPath を優先、なければ entries から projectPath を取得
        if "originalPath" in data:
            return data["originalPath"]
        entries = data.get("entries", [])
        if entries and "projectPath" in entries[0]:
            return entries[0]["projectPath"]
    except (json.JSONDecodeError, KeyError, IndexError):
        pass
    return None


def _parse_timestamp(ts_str: Optional[str]) -> Optional[datetime]:
    """ISO形式のタイムスタンプをパースする。"""
    if not ts_str:
        return None
    try:
        # "2026-01-30T03:17:44.781Z" のような形式
        return datetime.fromisoformat(ts_str.replace("Z", "+00:00"))
    except (ValueError, TypeError):
        return None


def _extract_text_from_content(content) -> str:
    """メッセージのcontent（stringまたはarray）からテキストを抽出する。

    thinkingブロックは除外し、textブロックのみ連結する。
    """
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        texts = []
        for block in content:
            if isinstance(block, dict) and block.get("type") == "text":
                texts.append(block.get("text", ""))
        return "\n".join(texts)
    return ""


def _extract_messages_from_content(content) -> list[dict]:
    """content配列からtool_useやtool_resultブロックを個別に抽出する。"""
    if not isinstance(content, list):
        return []
    blocks = []
    for block in content:
        if not isinstance(block, dict):
            continue
        block_type = block.get("type")
        if block_type in ("tool_use", "tool_result"):
            blocks.append(block)
    return blocks


def list_projects() -> list[ProjectInfo]:
    """プロジェクト一覧を返す。"""
    if not CLAUDE_PROJECTS_DIR.exists():
        return []

    projects = []
    for entry in sorted(CLAUDE_PROJECTS_DIR.iterdir()):
        if not entry.is_dir():
            continue
        dir_name = entry.name

        # 元のパスを取得
        original_path = _try_get_original_path(entry)
        if original_path is None:
            original_path = _decode_project_path(dir_name)

        # セッションファイル数をカウント
        session_count = sum(1 for f in entry.glob("*.jsonl"))

        projects.append(
            ProjectInfo(
                dir_name=dir_name,
                original_path=original_path,
                session_count=session_count,
            )
        )
    return projects


def list_sessions(project_name: str) -> list[SessionInfo]:
    """プロジェクト内のセッション一覧を返す。

    sessions-index.json がある場合はそれを利用し、
    ない場合はJSONLファイルを直接読み込んで情報を取得する。

    Args:
        project_name: プロジェクトのディレクトリ名（エンコード済み）
    """
    project_dir = CLAUDE_PROJECTS_DIR / project_name
    if not project_dir.exists():
        return []

    # sessions-index.json があれば利用
    index_path = project_dir / "sessions-index.json"
    if index_path.exists():
        sessions = _list_sessions_from_index(project_name, index_path)
        if sessions:
            return sessions

    # フォールバック: JSONLファイルを直接読む
    return _list_sessions_from_files(project_name, project_dir)


def _list_sessions_from_index(
    project_name: str, index_path: Path
) -> list[SessionInfo]:
    """sessions-index.json からセッション一覧を取得する。"""
    try:
        with open(index_path, encoding="utf-8") as f:
            data = json.load(f)
    except (json.JSONDecodeError, OSError):
        return []

    sessions = []
    for entry in data.get("entries", []):
        session_id = entry.get("sessionId", "")
        preview = entry.get("firstPrompt", "")
        if len(preview) > 200:
            preview = preview[:200] + "..."
        sessions.append(
            SessionInfo(
                session_id=session_id,
                project_name=project_name,
                preview=preview,
                timestamp=_parse_timestamp(entry.get("created")),
                message_count=entry.get("messageCount", 0),
                git_branch=entry.get("gitBranch", ""),
                summary=entry.get("summary", ""),
            )
        )

    # タイムスタンプで降順ソート（新しいものが先）
    sessions.sort(key=lambda s: s.timestamp or datetime.min, reverse=True)
    return sessions


def _list_sessions_from_files(
    project_name: str, project_dir: Path
) -> list[SessionInfo]:
    """JSONLファイルから直接セッション一覧を取得する。"""
    sessions = []
    for jsonl_file in project_dir.glob("*.jsonl"):
        session_id = jsonl_file.stem
        preview = ""
        timestamp = None
        git_branch = ""
        message_count = 0

        try:
            with open(jsonl_file, encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        obj = json.loads(line)
                    except json.JSONDecodeError:
                        continue

                    msg_type = obj.get("type")
                    if msg_type in ("user", "assistant"):
                        message_count += 1

                    if msg_type == "user" and not preview:
                        content = obj.get("message", {}).get("content", "")
                        preview = _extract_text_from_content(content)
                        if len(preview) > 200:
                            preview = preview[:200] + "..."
                        timestamp = _parse_timestamp(obj.get("timestamp"))
                        git_branch = obj.get("gitBranch", "")
        except OSError:
            continue

        sessions.append(
            SessionInfo(
                session_id=session_id,
                project_name=project_name,
                preview=preview,
                timestamp=timestamp,
                message_count=message_count,
                git_branch=git_branch,
            )
        )

    sessions.sort(key=lambda s: s.timestamp or datetime.min, reverse=True)
    return sessions


def load_session(project_name: str, session_id: str) -> list[Message]:
    """セッションの全メッセージを返す。

    Args:
        project_name: プロジェクトのディレクトリ名（エンコード済み）
        session_id: セッションのUUID
    """
    jsonl_path = CLAUDE_PROJECTS_DIR / project_name / f"{session_id}.jsonl"
    if not jsonl_path.exists():
        return []

    messages = []
    with open(jsonl_path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue

            msg_type = obj.get("type")
            timestamp = _parse_timestamp(obj.get("timestamp"))

            if msg_type == "user":
                content = obj.get("message", {}).get("content", "")
                # tool_result を含むユーザーメッセージ
                if isinstance(content, list):
                    tool_blocks = _extract_messages_from_content(content)
                    if tool_blocks:
                        for block in tool_blocks:
                            if block.get("type") == "tool_result":
                                result_content = block.get("content", "")
                                if isinstance(result_content, list):
                                    result_text = _extract_text_from_content(
                                        result_content
                                    )
                                else:
                                    result_text = str(result_content)
                                messages.append(
                                    Message(
                                        role="tool_result",
                                        text=result_text,
                                        timestamp=timestamp,
                                    )
                                )
                    else:
                        # テキストのみのユーザーメッセージ
                        text = _extract_text_from_content(content)
                        if text:
                            messages.append(
                                Message(role="user", text=text, timestamp=timestamp)
                            )
                else:
                    text = _extract_text_from_content(content)
                    if text:
                        messages.append(
                            Message(role="user", text=text, timestamp=timestamp)
                        )

            elif msg_type == "assistant":
                content = obj.get("message", {}).get("content", "")
                # テキスト部分を抽出
                text = _extract_text_from_content(content)
                if text:
                    messages.append(
                        Message(role="assistant", text=text, timestamp=timestamp)
                    )

                # tool_use ブロックを個別に追加
                if isinstance(content, list):
                    for block in content:
                        if isinstance(block, dict) and block.get("type") == "tool_use":
                            tool_name = block.get("name", "")
                            tool_input = block.get("input", {})
                            # ツール使用の要約テキスト
                            summary = _summarize_tool_use(tool_name, tool_input)
                            messages.append(
                                Message(
                                    role="tool_use",
                                    text=summary,
                                    timestamp=timestamp,
                                    tool_name=tool_name,
                                    tool_input=tool_input,
                                )
                            )

            elif msg_type == "system":
                subtype = obj.get("subtype", "")
                text = obj.get("message", {}).get("content", "")
                if isinstance(text, (list, dict)):
                    text = _extract_text_from_content(text)
                if not text:
                    text = f"[system: {subtype}]" if subtype else "[system]"
                messages.append(
                    Message(role="system", text=str(text), timestamp=timestamp)
                )

    return messages


def _summarize_tool_use(tool_name: str, tool_input: dict) -> str:
    """ツール使用の要約テキストを生成する。"""
    if tool_name == "Bash":
        cmd = tool_input.get("command", "")
        desc = tool_input.get("description", "")
        if desc:
            return f"[Bash] {desc}"
        if len(cmd) > 100:
            return f"[Bash] {cmd[:100]}..."
        return f"[Bash] {cmd}"
    elif tool_name == "Read":
        return f"[Read] {tool_input.get('file_path', '')}"
    elif tool_name == "Write":
        return f"[Write] {tool_input.get('file_path', '')}"
    elif tool_name == "Edit":
        return f"[Edit] {tool_input.get('file_path', '')}"
    elif tool_name == "Grep":
        pattern = tool_input.get("pattern", "")
        path = tool_input.get("path", ".")
        return f"[Grep] {pattern} in {path}"
    elif tool_name == "Glob":
        return f"[Glob] {tool_input.get('pattern', '')}"
    elif tool_name == "WebFetch":
        return f"[WebFetch] {tool_input.get('url', '')}"
    else:
        return f"[{tool_name}]"
