# 기능 카탈로그

Dev Cockpit 모든 기능의 정확한 동작 정의. 일상적인 사용법은 [USAGE.ko.md](USAGE.ko.md), 내부 구조는 [ARCHITECTURE.ko.md](ARCHITECTURE.ko.md) 참고.

*English version: [FEATURES.md](FEATURES.md)*

## 1. 런타임 감지

매 스캔 틱(기본 3초)마다:

1. `lsof -nP -iTCP -sTCP:LISTEN`으로 리스닝 중인 TCP 소켓을 나열합니다(field 모드, 1회 실행).
2. `sysinfo`로 소유 PID마다 이름, 전체 커맨드라인, 작업 디렉터리, CPU %, RSS, 시작 시각, 부모 체인을 확인합니다.
3. 휴리스틱 엔진이 커맨드라인·cwd·(컨테이너는) 이미지명으로 각 프로세스를 분류합니다.

서비스별 수집 항목: **포트 · PID · 프로세스명 · 커맨드 · 프로젝트 경로 · 프로젝트명 · 프레임워크 · 런타임 · CPU · 메모리 · 업타임 · 상태**.

### 인식하는 프레임워크·서비스

Node 생태계: Vite, Next.js, React (CRA / react-scripts), Nest.js, Express, Fastify, Koa, Hono, Nuxt, Svelte/SvelteKit, Vue, Angular, Astro, Electron, Tauri, Node, Bun, Deno.

Python: FastAPI, uvicorn, Django, Flask, Streamlit, 일반 Python.

데이터베이스·인프라: PostgreSQL(`postmaster`, pgvector), Redis(`redis-server`, Valkey), MySQL/MariaDB, MongoDB, ClickHouse, OpenSearch, Meilisearch, memcached, MinIO, RabbitMQ, Kafka(`cp-kafka`), LocalStack, nginx.

기타 런타임: Go, Java, .NET, PHP, Ruby.

그 외에도 포트를 열고 있으면 전부 표시됩니다 — 프로젝트 루트 아래에 있으면 일반 개발 프로세스로, 아니면 **Other listeners**(기본 숨김)로.

## 2. 프로젝트 디스커버리

- 설정된 루트 폴더(기본: `~/Dev`, `~/Developer`, `~/Projects`, `~/Code`, `~/repos`, `~/workspace`)를 깊이 제한 BFS로 스캔하며 `node_modules`, `.git` 내부, 빌드 캐시, `Library` 등은 건너뜁니다.
- `.git` 또는 매니페스트가 있으면 프로젝트로 인식: `package.json`, `pyproject.toml`, `requirements.txt`, `docker-compose.yml` / `docker-compose.yaml` / `compose.yml` / `compose.yaml`, `go.mod`, `Cargo.toml`, `Gemfile`, `mix.exs`.
- **모노레포** — `pnpm-workspace.yaml`의 패키지 glob과 `package.json`의 `workspaces`를 확장해, 서브 패키지마다 자체 시작 가능 서비스를 만듭니다(예: `web`, `api`).
- **매니페스트 기반 프레임워크 추론** — `package.json` / `pyproject.toml` / `requirements.txt`의 의존성으로 아무것도 실행 중이 아니어도 프로젝트에 태그를 붙입니다(Vite, Next, Nest, FastAPI, …).
- 재발견은 10분마다, 그리고 요청 시(트레이 → Rescan Now, 설정 → rescan) 실행됩니다.

## 3. 프로젝트 중심 뷰

링크 단계가 모든 데이터 소스를 하나의 스냅샷으로 결합합니다:

- 프로세스는 작업 디렉터리 포함 관계로 프로젝트에 연결되며, 부모 프로세스 체인을 거슬러 올라갑니다(`pnpm dev`의 자식 `node`는 프로젝트를 물려받음).
- 컨테이너는 Docker Compose 라벨(`com.docker.compose.project.working_dir`)을 프로젝트 경로와 매칭해 연결합니다.
- 각 프로젝트 카드에 표시: Git 브랜치, dirty 개수, ahead/behind, 마지막 커밋(요약 · 작성자 · 시점), 실행 중 서비스, 컨테이너, 리스닝 포트, compose 상태.
- 아무것도 실행하지 않는 프로젝트는 **Idle projects**로 묶이고, 숨긴 프로젝트(설정)는 모든 곳에서 제외됩니다.

## 4. 원클릭 컨트롤

| 액션 | 동작 |
| --- | --- |
| Start | 감지된 시작 명령을 사용자의 로그인 셸로, 독립 프로세스 그룹에서 실행하고 출력을 로그 세션으로 캡처. 명령 결정: lockfile → 패키지 매니저(`pnpm-lock.yaml`→pnpm, `yarn.lock`→yarn, `bun.lock(b)`→bun, 그 외 npm), 스크립트 우선순위 `dev` → `start:dev` → `serve` → `start`. Compose 프로젝트는 `docker compose up -d`. |
| Stop | SIGTERM; `⌥`클릭은 SIGKILL. 대상이 자기 프로세스 그룹의 리더면 그룹 전체에 시그널(`npm run dev` → `node` 트리를 깔끔히 종료). 감지된 개발 서비스가 아닌 프로세스에는 시그널을 거부. |
| Restart | 중지 → 종료 대기 → 알고 있는 명령으로 재시작; 실패는 토스트로 표시. |
| Open | 터미널(설정 가능, 기본 Terminal), 에디터(기본 Cursor), Finder, 브라우저(`http://localhost:<port>`). URL은 http/https만 허용. |
| Override | 프로젝트별 시작 명령 덮어쓰기와 표시 이름. 인라인 또는 설정에서 편집, config에 저장. |

사용자가 직접 중지한 것은 기록되어 "service stopped" 알림에서 제외됩니다.

## 5. Docker 통합

- 데몬 프로브 먼저 — Docker가 꺼져 있으면 모든 Docker 작업을 건너뛰고(에러 스팸 없음) 안내만 표시하며, 상태 전이(down↔up)는 알림으로 전달됩니다.
- `docker ps`를 5초마다(설정 가능): 이름, 이미지, 상태, health, 포트 바인딩, compose 프로젝트/서비스/작업 디렉터리 라벨.
- `docker stats`는 더 느린 주기(15초)로 CPU / 메모리 / limit을 채웁니다.
- 액션: 컨테이너별 start / stop / restart; `docker logs -f` 실시간 tail(최근 300줄 + follow); 프로젝트별 compose up/down과 스트리밍 출력.
- Compose 컨테이너는 자동으로 해당 프로젝트 카드로 묶입니다.

## 6. 포트 관리·충돌

- 리스닝 중인 모든 TCP 포트를 바인드 주소와 함께 추적합니다(`*:5173`, `127.0.0.1:8080`, `[::1]:6379`).
- **충돌 감지**는 주소 의미론을 이해합니다: 바인드가 겹칠 때만 충돌(와일드카드 `*`는 그 포트의 모든 주소와 겹침; 서로 다른 루프백 주소끼리는 겹치지 않음). 한 서버의 IPv4/IPv6 이중 바인드는 오탐이 아닙니다.
- 충돌은 각 프로세스·PID·프로젝트·정확한 바인드를 명시한 빨간 배너로 표시되고 항목별 stop 버튼이 있으며, 처음 나타날 때 알림이 발송됩니다.

## 7. 헬스 모니터링

"프로세스 존재" 그 이상의 단계:

- **TCP 프로브** — 짧은 타임아웃으로 포트에 연결(IPv4 후 IPv6).
- **HTTP 프로브** — HTTP를 말하는 서비스에 `GET /` 후 상태 라인 파싱. 2xx–4xx 응답은 정상으로 간주(4xx도 "응답함"의 의미); 5xx 응답은 degraded로 표시.
- 컨테이너 health 상태(`healthy` / `starting` / `unhealthy`)는 Docker에서 가져옵니다.
- 결과 단계: `ok` · `warn`(TCP는 열렸는데 HTTP 5xx / 컨테이너 starting) · `down`(포트가 연결을 받지 않음) · `unknown`.
- 헬스 실패 알림은 정상이던 서비스가 **연속 2회** 프로브에 실패했을 때만 발송되고, 복구는 1회 알림(둘 다 토글 가능).

## 8. macOS 알림

| 이벤트 | 발송 조건 |
| --- | --- |
| Service stopped | 포트를 가진 추적 서비스가 사용자 조작 없이 사라짐 |
| Container stopped | 실행 중이던 컨테이너가 예기치 않게 `running`을 벗어남 |
| Health check failed | 정상이던 서비스가 연속 2회 프로브 실패 |
| Recovered | 다운 알림이 나갔던 서비스가 다시 정상화 |
| Port conflict | 새 리스너 겹침 충돌 발생 |
| Docker unreachable / back | 데몬 상태 전이 |

안전장치: 이벤트 키별 쿨다운(기본 60초, 10초~1시간 범위), 시작 grace(처음 두 틱), 슬립/웨이크 재동기화 grace, 사용자 중지 억제. 릴리즈 빌드는 네이티브 알림 센터, 개발 빌드는 `osascript`로 대체.

## 9. 로그

- 소스: Dev Cockpit이 시작한 프로세스(stdout+stderr)와 `docker logs -f` tail.
- 링 버퍼: 세션당 2,000줄, 줄당 4,000자 — 구조적으로 메모리가 제한됩니다.
- 줄은 Tauri 이벤트로 배치 전송되며, 뷰는 필터, 일시정지(재개 시 따라잡기), 클리어, 복사, stderr 강조, 자동 follow를 지원합니다.
- 뷰를 닫으면 세션이 정리되고 컨테이너 tail이 종료됩니다. 디스크에는 아무것도 기록하지 않습니다.

## 10. 검색

검색창 하나로 스냅샷 전체를 필터링합니다 — 프로젝트명, 경로, 포트, PID, 프로세스명, 커맨드, 프레임워크, 런타임, 컨테이너명, 이미지, compose 서비스, Git 브랜치. 대소문자 무시 부분 문자열 매칭이며, 비어 있는 섹션은 접힙니다.

## 11. 상시 표시 UX

- **트레이 아이콘**에 실행 중 서비스+컨테이너 개수를 실시간 표시; 툴팁에 서비스/컨테이너/포트/충돌 내역.
- **팝오버 패널**(460×640)은 트레이 아이콘 아래에 고정되고 트레이가 있는 모니터 안으로 클램프되며, 일반 창 위에 뜨고 Dock·앱 전환기에는 나타나지 않습니다.
- 기본은 블러 시 숨김; **핀**으로 유지. `⌃⌥D`로 어디서든 토글(전역 단축키).
- **단일 인스턴스** — 재실행하면 복제 대신 기존 인스턴스가 포커스됩니다.
- 시스템을 따르는(또는 고정 가능한) 다크/라이트 테마.
- 스캔 결과가 실제로 바뀌었을 때만 UI를 다시 렌더링합니다.

## 12. 안정성·성능

- 모든 외부 명령(`lsof`, `docker`, `git`, 실행한 개발 서버)은 타임아웃과 함께 실행됩니다; 수집기 하나가 실패해도 앱이 죽지 않고 우아하게 성능이 저하됩니다(에러는 상태바에 표시).
- 슬립/웨이크 감지: 스케줄러가 멈췄다 깨어나면 재동기화 사이클이 실행되어 가짜 "stopped" 알림이 절대 발생하지 않습니다.
- 주기 분리(포트 3초 / docker 5초 / stats 15초 / git 20초 / 디스커버리 10분)로 평상시 CPU를 ~1% 미만으로 유지합니다.
- 스냅샷 diff로 변화가 없으면 UI 작업이 0이며, 로그 버퍼와 세션별 상한으로 메모리를 제한합니다.
