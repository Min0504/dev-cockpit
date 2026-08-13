# 아키텍처

Dev Cockpit의 구성: 모듈, 데이터 흐름, IPC 표면, 그리고 그 뒤의 설계 결정들.

*English version: [ARCHITECTURE.md](ARCHITECTURE.md)*

## 개요

```
┌─────────────────────────────  macOS  ─────────────────────────────┐
│  lsof        sysinfo       docker CLI      git CLI     filesystem │
└────┬────────────┬──────────────┬──────────────┬────────────┬─────┘
     │            │              │              │            │
┌────▼────────────▼──────────────▼──────────────▼────────────▼─────┐
│                     scan/  (collectors, pure functions)          │
│   ports.rs      procs.rs      docker.rs      git.rs   projects.rs│
│        └───────────┬─ detect.rs (classification) ─┬──────┘       │
│                    └──────── link.rs ─────────────┘              │
│                       (assemble Snapshot)                        │
├──────────────────────────────────────────────────────────────────┤
│ monitor.rs   scheduler · health merge · diff · tray · notifier   │
│ control.rs   start/stop/restart · compose · open                 │
│ logs.rs      ring-buffer log sessions · event streaming          │
│ notify.rs    snapshot diff → Notification Center (cooldowns)     │
│ state.rs     shared AppState (config, snapshot, managed procs)   │
│ commands.rs  #[tauri::command] surface (thin wrappers)           │
│ lib.rs       tray · panel window · global shortcut · plugins     │
└───────────────┬──────────────────────────────────────────────────┘
                │  Tauri IPC: invoke (commands) + emit (events)
┌───────────────▼──────────────────────────────────────────────────┐
│  React UI (src/)                                                 │
│  ipc.ts → hooks.ts → App.tsx → ProjectCard / ServiceRow /        │
│  ContainerRow / LogsView / SettingsView                          │
└──────────────────────────────────────────────────────────────────┘
```

스택: **Tauri v2**(Rust 백엔드, WKWebView 셸) + **React 19 / TypeScript strict** + 순수 CSS. React 외 웹 프레임워크 없음, CSS 프레임워크 없음, ORM 없음 — 가장 무거운 의존성이 `sysinfo`입니다.

## 백엔드 모듈 (`src-tauri/src/`)

| 모듈 | 책임 |
| --- | --- |
| `models.rs` | 모든 공유 타입(`Snapshot`, `Service`, `Container`, `ProjectView`, `AppConfig`, …). serde `camelCase`로 UI에 직렬화. 데이터 계약의 단일 진실 공급원. |
| `config.rs` | `~/Library/Application Support/<id>/` 아래 `AppConfig` JSON 로드/저장, 기본 프로젝트 루트, 값 범위 제한. |
| `scan/ports.rs` | `lsof` field 모드 1회 실행 → 바인드 주소를 포함한 리스너 목록; 충돌 감지를 위한 주소 겹침 로직(와일드카드 vs 루프백 vs 특정 주소, IPv4/IPv6). |
| `scan/procs.rs` | 리스너 PID와 부모 체인의 `sysinfo` 메트릭(CPU, RSS, cwd, 커맨드라인, 시작 시각); 프로젝트 연결용 조상 관계 헬퍼. |
| `scan/detect.rs` | 분류 휴리스틱: 커맨드라인 + cwd + 이미지명 → 프레임워크 키, 런타임, 표시 이름, HTTP 여부. 시스템 데몬 노이즈 필터. |
| `scan/projects.rs` | 루트 폴더 BFS 디스커버리(깊이 제한, 스킵 목록), 매니페스트 파싱, 모노레포 워크스페이스 확장, 패키지 매니저 + 시작 스크립트 결정. 디스커버리 틱 사이에는 캐시. |
| `scan/docker.rs` | 데몬 프로브, `docker ps`(커스텀 구분자 포맷), `docker stats`, compose 라벨 추출, 컨테이너 액션. |
| `scan/git.rs` | `git status --porcelain=v2 --branch` + `git log -1` 파싱 → 브랜치, dirty, ahead/behind, 마지막 커밋. |
| `scan/health.rs` | 의존성 없는 TCP connect와 최소 HTTP/1.1 `GET /` 프로브. 엄격한 타임아웃, 블로킹 스레드에서 실행. |
| `scan/link.rs` | 조인: 리스너 × 프로세스 × 프로젝트 × 컨테이너 × git → `Snapshot`. cwd/조상 관계 기반 프로젝트 연결, compose 링크, 충돌 그룹핑, 합계. |
| `monitor.rs` | 심장 박동. 소스별 주기의 틱 루프, 슬립/웨이크 재동기화, 병렬 헬스 프로브, 트레이 타이틀/툴팁 갱신, 알림 공급, 변경 시에만 스냅샷 emit. |
| `control.rs` | Start(로그인 셸 spawn, 독립 프로세스 그룹, 로그 세션), Stop(SIGTERM/SIGKILL, 그룹 인지, 개발 서비스 한정 가드), Restart, compose 액션, 터미널/에디터/Finder/URL 열기(http(s)만). |
| `logs.rs` | 로그 세션: 자식 stdout/stderr 또는 `docker logs -f --tail 300`을 제한된 링 버퍼(2,000줄 × 4,000자)로 펌핑, `logs://<id>` 이벤트로 배치 emit. |
| `notify.rs` | 스냅샷 differ → 알림 결정: 중지된 서비스/컨테이너, 헬스 실패(2회 연속) / 복구, 새 충돌, docker 데몬 전이. 키별 쿨다운, 시작 + 재동기화 grace, 사용자 중지 억제. 릴리즈는 네이티브 알림, 개발은 `osascript`. |
| `state.rs` | `AppState`: config, 최신 스냅샷, 관리 자식 프로세스 레지스트리, 로그 레지스트리, 억제 목록, 스케줄러 wake 핸들. |
| `commands.rs` | 약 20개의 `#[tauri::command]` 래퍼 — 자체 로직 없음. |
| `lib.rs` | 앱 배선: 트레이 아이콘 + 메뉴, 트레이 아래 패널 배치(모니터 클램프), 블러 시 숨김 vs 핀, `⌃⌥D` 전역 단축키, 자동 시작, 단일 인스턴스, 플러그인 등록. |

## 스케줄링

tokio 태스크 하나가 전부를 구동합니다(`monitor.rs`). 소스마다 자체 주기가 있고 시간이 되었을 때만 실행됩니다:

| 소스 | 기본 주기 |
| --- | --- |
| 포트 + 프로세스 (+ 헬스) | 3초 |
| Docker `ps` | 5초 |
| Docker `stats` | 15초 |
| Git | 20초 |
| 프로젝트 디스커버리 | 10분 |

- 루프는 약 800ms 단위로 잠들고, `Notify` 핸들로 커맨드(Scan Now, 설정 변경, 서비스 시작)가 즉시 깨울 수 있습니다.
- **슬립/웨이크**: 루프가 예산보다 훨씬 길게 잤다고 감지하면 다음 사이클은 *재동기화* — 데이터는 갱신하되 사라짐 알림은 억제하므로, 노트북을 덮었다 열어도 "service stopped" 알림이 쏟아지지 않습니다.
- **일시정지**(트레이 또는 상태바)는 루프를 완전히 세웁니다.

## 데이터 흐름

1. 수집기들이 실행되고(틱마다 시간이 된 것만) 순수 데이터를 만듭니다.
2. `link.rs`가 불변 `Snapshot { projects, orphanServices, otherListeners, unlinkedContainers, conflicts, totals, errors, seq }`을 조립합니다.
3. 헬스 결과가 병합되고 전이가 추적됩니다.
4. `notify.rs`가 이전 스냅샷과 diff하여 쿨다운을 거쳐 알림을 발송합니다.
5. 스냅샷을 마지막으로 emit한 것과 비교해 — **다를 때만** UI로 `snapshot`을 emit하고 트레이 타이틀을 갱신합니다.
6. React는 `useSnapshot()` 훅 하나로 받고, UI 전체는 (스냅샷, 설정, 뷰 상태)의 순수 함수입니다.

수집기 실패는 절대 전파되지 않습니다: 외부 명령마다 타임아웃이 있고, 에러는 `snapshot.errors`로 모여 상태바 경고로 표시되며, 나머지 데이터는 계속 살아 있습니다.

## IPC 표면

커맨드 (`src/ipc.ts`에서 invoke):

| 그룹 | 커맨드 |
| --- | --- |
| 데이터 | `get_snapshot`, `force_scan`, `rescan_projects` |
| 설정 | `get_config`, `set_config` |
| 서비스 | `start_service`, `stop_service`, `restart_service` |
| Docker | `docker_action`, `compose_action` |
| 열기 | `open_path` (터미널/에디터/finder), `open_url` (http/https만) |
| 로그 | `open_log_session`, `get_log_lines`, `close_log_session` |
| 앱 | `get_autostart`, `set_autostart`, `set_paused`, `is_paused`, `hide_panel`, `quit_app` |

이벤트 (백엔드 → UI): `snapshot`, `toast`, `paused`, `managed-exited`, `logs://<session>`, `logs-ended://<session>`.

## 프론트엔드 (`src/`)

| 파일 | 역할 |
| --- | --- |
| `types.ts` | `models.rs`의 TypeScript 미러(camelCase 계약). |
| `ipc.ts` | 타입이 붙은 `invoke`/`listen` 래퍼 — Tauri API를 만지는 유일한 파일. |
| `hooks.ts` | `useSnapshot`, `useConfig`, `useToasts`, `usePaused`, 시계 + 테마 훅. |
| `utils.ts` | 포매팅(바이트/CPU/업타임/상대 시간), 검색 매칭, 프레임워크 색상 + 모노그램. 단위 테스트 있음. |
| `App.tsx` | 셸: 검색, 충돌 배너, 섹션 구성, 시트 라우팅(로그/설정), 키보드 처리, 토스트. |
| `components/` | `ProjectCard`, `ServiceRow`, `ContainerRow`, `LogsView`, `SettingsView`. |
| `styles.css` | 디자인 토큰(테마별 CSS 변수) + 모든 컴포넌트 스타일. CSS 프레임워크 없음. |

UI 원칙: 장식보다 정보 밀도, hover로 드러나는 액션, 기능적 전환 외 애니메이션 없음, 메트릭에 tabular numerals, 시스템 폰트 스택, vibrancy가 적용된 네이티브 느낌의 팝오버.

## 안전 모델

설계상 로컬 전용 — 그래도 무엇을 건드릴 수 있는지 신중하게 정합니다:

- **Stop 가드**: 현재 스냅샷이 개발 서비스로 분류한 PID에만 시그널 가능; 시스템 프로세스는 거부.
- **프로세스 그룹**: 관리 자식은 독립 그룹에서 실행; stop은 그룹에 시그널하므로 고아 watcher가 남지 않음.
- **명령 실행**: Start는 감지된 패키지 매니저 스크립트 / compose, 또는 사용자가 명시한 override만 실행 — 스캔에서 나온 임의 문자열은 절대 실행하지 않음.
- **URL 가드**: `open_url`은 `http(s)`만 허용.
- **네트워크 송신 없음**: 프로브는 오직 `localhost` 대상; 어디에도 데이터를 보내지 않음. 로그는 메모리에만 유지.

## 테스트

- **Rust (25개)**: `lsof` field 모드 파싱 + 충돌 겹침, `docker ps` 파싱 + 메모리/포트 파싱, git porcelain 파싱, 임시 디렉터리 기반 디스커버리/워크스페이스/패키지 매니저 결정, 링크/연결/충돌/숨긴 프로젝트 조립, 실제 로컬 리스너 대상 헬스 프로브.
- **TypeScript (vitest)**: 포매팅과 검색 매칭 유틸리티.
- 파서는 의도적으로 문자열에 대한 순수 함수로 작성되어, 실제 CLI 없이도 픽스처로 엣지 케이스를 커버합니다.

## 빌드

- `npm run tauri dev` — vite 개발 서버 + 핫 리로드가 되는 디버그 Rust 빌드.
- `npm run tauri build` — 릴리즈 번들(`Dev Cockpit.app`, 약 7 MB).
- 아이콘은 `scripts/render-icons.swift`로 생성합니다(앱 스쿼클 + 메뉴바 템플릿 이미지).
