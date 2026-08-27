"""Streaming and query-envelope helpers shared by the high-level Torii client."""

from __future__ import annotations

import json
import time
from typing import Any, Callable, Dict, Iterable, List, Mapping, Optional, Union

import requests

from .query import AggregateSpec, ensure_aggregate
from .stream_events import EventCursor, SseEvent, SseStreamError


def create_torii_client_streaming_query_mixin(
    *,
    require_crypto: Callable[[], Any],
    expect_sorafs_reputation_status: Callable[..., None],
    normalize_count_mode_arg: Callable[[Optional[str]], Optional[str]],
    normalize_optional_string: Callable[[Any, str], Optional[str]],
) -> Any:
    """Bind client-local hooks to the reusable streaming/query implementation."""

    _require_crypto = require_crypto
    _expect_sorafs_reputation_status = expect_sorafs_reputation_status
    _normalize_count_mode_arg = normalize_count_mode_arg
    _normalize_optional_string = normalize_optional_string

    class ToriiClientStreamingQueryMixin:
        _base_url: str

        @staticmethod
        def _maybe_json(response: requests.Response) -> Optional[Any]:
            if not hasattr(response, "content"):
                try:
                    return response.json()
                except ValueError:
                    return getattr(response, "text", "") or None
            if not response.content:
                return None
            try:
                return response.json()
            except ValueError:
                return response.text or None

        @staticmethod
        def _maybe_transaction_receipt(response: requests.Response) -> Optional[Any]:
            content_type = response.headers.get("Content-Type", "")
            if "application/x-norito" not in content_type.lower():
                return None
            if not response.content:
                return None
            try:
                crypto = _require_crypto()
            except RuntimeError:
                return None
            if not hasattr(crypto, "decode_transaction_receipt_json"):
                return None
            try:
                receipt_json = crypto.decode_transaction_receipt_json(response.content)
            except Exception:
                return None
            try:
                return json.loads(receipt_json)
            except json.JSONDecodeError:
                return None

        @staticmethod
        def _parse_sse_event(
            lines: Iterable[str],
            *,
            decode_json: bool = True,
            json_loader: Optional[Callable[[str], Any]] = None,
        ) -> Optional[SseEvent]:
            raw_lines = list(lines)
            data_chunks: List[str] = []
            event_name: Optional[str] = None
            event_id: Optional[str] = None
            retry_value: Optional[int] = None
            for entry in raw_lines:
                if entry.startswith(":"):
                    continue
                field, sep, value = entry.partition(":")
                value = value.lstrip() if sep else ""
                if field == "data":
                    data_chunks.append(value)
                elif field == "id":
                    event_id = value or None
                elif field == "event":
                    event_name = value or None
                elif field == "retry":
                    try:
                        retry_value = int(value)
                    except ValueError:
                        retry_value = None
            if not data_chunks and event_name is None and event_id is None and retry_value is None:
                return None
            payload: Any
            if data_chunks:
                joined = "\n".join(data_chunks)
                if decode_json:
                    if json_loader is not None:
                        payload = json_loader(joined)
                    else:
                        try:
                            payload = json.loads(joined)
                        except json.JSONDecodeError:
                            payload = joined
                else:
                    payload = joined
            else:
                payload = None
            return SseEvent(
                event=event_name,
                data=payload,
                id=event_id,
                retry=retry_value,
                raw="\n".join(raw_lines),
            )

        def _stream_sse(
            self,
            path: str,
            *,
            params: Optional[Mapping[str, Any]] = None,
            headers: Optional[Mapping[str, str]] = None,
            headers_factory: Optional[Callable[[], Mapping[str, str]]] = None,
            timeout: Optional[float] = None,
            max_retries: Optional[int] = 3,
            backoff_base: float = 0.5,
            last_event_id: Optional[str] = None,
            resume: bool = False,
            decode_json: bool = True,
            cursor: Optional[EventCursor] = None,
            allow_resume: bool = False,
            allow_redirects: bool = True,
            strict_utf8: bool = False,
            maximum_event_bytes: Optional[int] = None,
            json_loader: Optional[Callable[[str], Any]] = None,
            on_event: Optional[Callable[[SseEvent], None]] = None,
            expected_content_type: Optional[str] = None,
            require_identity_encoding: bool = False,
            payload_free_errors: bool = False,
        ):
            url = f"{self._base_url}{path}"
            if headers is not None and headers_factory is not None:
                raise ValueError("_stream_sse accepts only one of headers or headers_factory")
            if not allow_resume and (last_event_id is not None or resume or cursor is not None):
                raise ValueError(f"{path} does not support SSE replay")
            active_last_id = (
                last_event_id
                if last_event_id is not None
                else (cursor.last_event_id if cursor is not None else None)
            )
            should_resume = allow_resume and (resume or last_event_id is not None or cursor is not None)

            def iterator():
                nonlocal active_last_id

                def process_event(event: SseEvent) -> SseEvent:
                    nonlocal active_last_id
                    if event.event == "stream_error":
                        raise SseStreamError.from_event(event)
                    if event.id is not None and allow_resume:
                        active_last_id = event.id
                        if cursor is not None:
                            cursor.advance(event)
                    if on_event is not None:
                        on_event(event)
                    return event

                attempt = 0
                backoff = max(backoff_base, 0.0)
                while True:
                    try:
                        final_headers: Dict[str, str] = dict(self._default_headers)
                        final_headers.pop("Accept", None)
                        attempt_headers = headers_factory() if headers_factory is not None else headers
                        if attempt_headers:
                            final_headers.update(attempt_headers)
                        if not allow_resume:
                            for name in tuple(final_headers):
                                if name.lower() == "last-event-id":
                                    final_headers.pop(name)
                        final_headers.setdefault("Accept", "text/event-stream")
                        if should_resume and active_last_id:
                            final_headers["Last-Event-ID"] = active_last_id
                        with self._session.get(
                            url,
                            params=params,
                            headers=final_headers or None,
                            stream=True,
                            timeout=timeout,
                            allow_redirects=allow_redirects,
                        ) as response:
                            if payload_free_errors:
                                _expect_sorafs_reputation_status(
                                    response,
                                    {200},
                                    path,
                                )
                            else:
                                self._expect_status(response, {200})
                            if require_identity_encoding:
                                content_encoding = response.headers.get("Content-Encoding")
                                if (
                                    content_encoding is not None
                                    and content_encoding.lower() != "identity"
                                ):
                                    raise ValueError(
                                        f"{path} Content-Encoding must be identity"
                                    )
                            if expected_content_type is not None:
                                content_type = response.headers.get("Content-Type")
                                if (
                                    content_type is None
                                    or content_type.split(";", 1)[0].strip().lower()
                                    != expected_content_type
                                ):
                                    raise ValueError(
                                        f"{path} Content-Type must be "
                                        f"{expected_content_type}"
                                    )
                            attempt = 0
                            backoff = max(backoff_base, 0.0)
                            buffer: list[str] = []
                            buffered_bytes = 0
                            first_line = True
                            for raw_line in response.iter_lines(decode_unicode=not strict_utf8):
                                if raw_line is None:
                                    continue
                                if strict_utf8:
                                    if not isinstance(raw_line, (bytes, bytearray, memoryview)):
                                        raise ValueError(
                                            f"{path} SSE body yielded a non-byte line"
                                        )
                                    raw_bytes = bytes(raw_line)
                                    if first_line and raw_bytes.startswith(b"\xef\xbb\xbf"):
                                        raise ValueError(
                                            f"{path} SSE body must not contain a UTF-8 BOM"
                                        )
                                    try:
                                        decoded_line = raw_bytes.decode("utf-8", "strict")
                                    except UnicodeDecodeError as exc:
                                        raise ValueError(
                                            f"{path} SSE body must be strict UTF-8"
                                        ) from exc
                                    line_bytes = len(raw_bytes) + 1
                                else:
                                    decoded_line = raw_line
                                    line_bytes = len(str(raw_line).encode("utf-8")) + 1
                                first_line = False
                                buffered_bytes += line_bytes
                                if (
                                    maximum_event_bytes is not None
                                    and buffered_bytes > maximum_event_bytes
                                ):
                                    raise ValueError(
                                        f"{path} SSE event exceeds its "
                                        f"{maximum_event_bytes}-byte size bound"
                                    )
                                line = decoded_line if strict_utf8 else decoded_line.strip()
                                if not line:
                                    if buffer:
                                        event = self._parse_sse_event(
                                            buffer,
                                            decode_json=decode_json,
                                            json_loader=json_loader,
                                        )
                                        buffer.clear()
                                        if event is None:
                                            buffered_bytes = 0
                                            continue
                                        yield process_event(event)
                                    buffered_bytes = 0
                                    continue
                                buffer.append(line)
                            if buffer:
                                event = self._parse_sse_event(
                                    buffer,
                                    decode_json=decode_json,
                                    json_loader=json_loader,
                                )
                                buffer.clear()
                                if event is not None:
                                    yield process_event(event)
                            break
                    except requests.RequestException:
                        attempt += 1
                        if max_retries is not None and attempt > max_retries:
                            raise
                        if backoff > 0.0:
                            time.sleep(backoff)
                            backoff *= 2
                        continue

            return iterator()

        @staticmethod
        def _build_query_envelope(
            *,
            filter: Optional[Mapping[str, Any]] = None,
            select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
            sort: Optional[Any] = None,
            limit: Optional[int] = None,
            offset: Optional[int] = None,
            fetch_size: Optional[int] = None,
            count_mode: Optional[str] = None,
            query_name: Optional[str] = None,
            aggregate: Optional[Union[AggregateSpec, Mapping[str, Any]]] = None,
        ) -> Dict[str, Any]:
            body: Dict[str, Any] = {}
            if filter is not None:
                body["filter"] = dict(filter)
            normalized_select = ToriiClientStreamingQueryMixin._normalize_query_select(select)
            if normalized_select is not None:
                body["select"] = normalized_select
            if sort is not None:
                body["sort"] = sort
            pagination: Dict[str, int] = {}
            if limit is not None:
                pagination["limit"] = int(limit)
            if offset is not None:
                pagination["offset"] = int(offset)
            if pagination:
                body["pagination"] = pagination
            if fetch_size is not None:
                body["fetch_size"] = int(fetch_size)
            if count_mode is not None:
                body["count_mode"] = _normalize_count_mode_arg(count_mode)
            query_name_value = _normalize_optional_string(query_name, "query_name")
            if query_name_value is not None:
                body["query"] = query_name_value
            aggregate_value = ensure_aggregate(aggregate)
            if aggregate_value is not None:
                if normalized_select is not None:
                    raise ValueError("select and aggregate are mutually exclusive")
                body["aggregate"] = aggregate_value
            return body

        @staticmethod
        def _normalize_query_select(
            select: Optional[Iterable[Union[str, Mapping[str, Any]]]],
        ) -> Optional[List[Union[str, Dict[str, Any]]]]:
            if select is None:
                return None
            if isinstance(select, (str, bytes, bytearray)):
                raise TypeError("select must be a sequence of field paths or objects")
            normalized: List[Union[str, Dict[str, Any]]] = []
            for index, entry in enumerate(select):
                if isinstance(entry, str):
                    field_path = entry.strip()
                    if not field_path:
                        raise ValueError(f"select[{index}] must be a non-empty field path")
                    normalized.append(field_path)
                elif isinstance(entry, Mapping):
                    normalized.append(dict(entry))
                else:
                    raise TypeError(f"select[{index}] must be a field-path string or mapping")
            return normalized

        @staticmethod
        def _ensure_no_query_args(
            *,
            envelope: Mapping[str, Any],
            filter: Optional[Mapping[str, Any]],
            select: Optional[Iterable[Union[str, Mapping[str, Any]]]],
            sort: Optional[Any],
            limit: Optional[int],
            offset: Optional[int],
            fetch_size: Optional[int],
            count_mode: Optional[str],
            query_name: Optional[str],
            aggregate: Optional[Union[AggregateSpec, Mapping[str, Any]]],
        ) -> None:
            if any(
                value is not None
                for value in (
                    filter,
                    select,
                    sort,
                    limit,
                    offset,
                    fetch_size,
                    count_mode,
                    query_name,
                    aggregate,
                )
            ):
                raise ValueError(
                    "provide either `envelope` or builder arguments "
                    "(filter/select/sort/limit/offset/fetch_size/count_mode/query_name/aggregate), "
                    "not both"
                )

    return ToriiClientStreamingQueryMixin
