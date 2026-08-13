# Dev Cockpit

macOS 메뉴바에 상주하는 **로컬 개발 환경 대시보드**.

> 터미널을 뒤져서 지금 무엇이 실행 중인지 확인하지 않는다.
> 화면을 보면 현재 개발 환경을 바로 알 수 있어야 한다.

리스닝 포트, 개발 프로세스, Docker 컨테이너, Git 상태를 실시간으로 감지해 **프로젝트 단위로 묶어서** 보여주고, 시작/중지/재시작/로그/compose up·down까지 팝오버 패널에서 바로 조작합니다.

**Port Monitor + Process Monitor + Project Dashboard + Docker Dashboard + Git Status + Service Controller + Health Monitor + macOS Notification** — 하나의 가벼운 메뉴바 앱(번들 약 7MB, 유휴 CPU 1% 미만).

*English version: [README.md](README.md)*

## 기능

- **실시간 감지** — `lsof` 포트 스캔 + `sysinfo` 프로세스 메트릭(CPU/메모리/업타임). 명령줄·작업 디렉터리·Docker 이미지 휴리스틱으로 Vite, Next.js, NestJS, FastAPI, Postgres, Redis 등 40여 종을 자동 식별.
- **프로젝트 중심 뷰** — 개발 폴더에서 Git 저장소/매니페스트 기준으로 프로젝트를 발견하고, 실행 중인 서비스·컨테이너·포트를 카드로 그룹핑. 브랜치, dirty/ahead/behind, 마지막 커밋 표시.
- **원클릭 컨트롤** — Start(감지된 `dev` 스크립트/`docker compose up`), Stop(SIGTERM, ⌥클릭 시 SIGKILL), Restart, 터미널/에디터/Finder/브라우저 열기.
- **Docker 통합** — 컨테이너 상태·포트·CPU/메모리, start/stop/restart, 실시간 로그, Compose 프로젝트 자동 연결과 up/down.
- **포트 충돌 감지** — 같은 포트에 바인드가 겹치는 리스너가 생기면 배너로 표시, 프로세스/프로젝트 명시.
- **헬스 모니터링** — TCP/HTTP 프로브로 "프로세스는 살아있지만 응답하지 않는" 상태를 구분.
- **macOS 알림** — 예기치 않은 종료, 헬스 실패/복구, 포트 충돌, Docker 데몬 상태. 이벤트별 쿨다운 + 슬립/웨이크 grace로 알림 폭주 방지.
- **로그 뷰** — 실시간 스트림, 필터/일시정지/클리어, 링버퍼로 메모리 제한.
- **검색** — 프로젝트명·포트·프로세스·프레임워크·컨테이너·브랜치 통합 검색.

## 설치

[Releases](https://github.com/Min0504/dev-cockpit/releases)에서 최신 `Dev Cockpit.app`을 내려받아(Apple Silicon) `/Applications`로 옮기고 실행하세요. 서명되지 않은 앱이라 첫 실행은 우클릭 → 열기로 진행합니다.

## 실행 방법

### 요구사항

- macOS 12+
- [Rust](https://rustup.rs) (stable), Node 20+ (npm 포함)
- Docker 기능은 데몬(Docker Desktop, OrbStack 등) 실행 중일 때 자동 활성화 — 없어도 앱은 정상 동작

### 빌드

```bash
npm install

# 개발 모드 (핫 리로드)
npm run tauri dev

# 릴리즈 번들
npm run tauri build
# → src-tauri/target/release/bundle/macos/Dev Cockpit.app
```

`.app`을 `/Applications`로 옮기고 실행하면 메뉴바에 게이지 아이콘이 나타납니다. 트레이 메뉴나 설정에서 **Launch at Login**을 켜두면 항상 사용할 수 있습니다.

서명되지 않은 빌드이므로 첫 실행 시 우클릭 → 열기로 Gatekeeper를 통과해야 할 수 있습니다.

## 사용법 요약

| 동작 | 방법 |
| --- | --- |
| 패널 열기/닫기 | 메뉴바 아이콘 클릭 또는 `⌃⌥D` |
| 검색 | 패널에서 `/` 또는 `⌘F` |
| 뒤로/닫기 | `Esc` |
| 강제 종료 | Stop 버튼 `⌥`클릭 |
| HTTP 서비스 열기 | 포트 배지 클릭 |
| 패널 고정 | 핀 버튼 (블러 시 숨김 해제) |
| 모니터링 일시정지 | 트레이 메뉴 → Pause Monitoring |

트레이 아이콘 옆 숫자는 실행 중인 서비스+컨테이너 개수이고, 툴팁에서 서비스/컨테이너/포트/충돌 내역을 보여줍니다.

자세한 내용: **[docs/USAGE.ko.md](docs/USAGE.ko.md)**

## 설정

모든 설정은 앱 내 Settings에서 편집하며 아래 파일에 저장됩니다.

```
~/Library/Application Support/com.minseokchae.devcockpit/config.json
```

- 프로젝트 루트 (기본: `~/Dev`, `~/Developer`, `~/Projects`, `~/Code`, `~/repos`, `~/workspace` 중 존재하는 폴더)
- 스캔 주기 (포트/프로세스 기본 3초, Docker 5초, Git 20초, 디스커버리 10분)
- 알림 이벤트별 on/off + 쿨다운
- 프로젝트 숨기기/이름 변경, 시작 명령 override
- 테마(시스템/라이트/다크), 로그인 시 자동 시작

## 문서

| 문서 | 내용 |
| --- | --- |
| [docs/USAGE.ko.md](docs/USAGE.ko.md) | 설치, 패널 사용법, 모든 설정 설명 ([English](docs/USAGE.md)) |
| [docs/FEATURES.ko.md](docs/FEATURES.ko.md) | 기능별 상세 동작 ([English](docs/FEATURES.md)) |
| [docs/ARCHITECTURE.ko.md](docs/ARCHITECTURE.ko.md) | 모듈 구조, 데이터 흐름, IPC, 설계 결정 ([English](docs/ARCHITECTURE.md)) |

## 개발

```bash
npm run typecheck     # tsc --noEmit
npm test              # vitest (프론트엔드 유닛 테스트)
cargo test            # Rust 테스트 (src-tauri/ 에서)
cargo clippy          # Rust 린트 (src-tauri/ 에서)
```

스택: Tauri v2 (Rust) + React 19 / TypeScript strict + vanilla CSS. 런타임 의존성은 시스템 `lsof`·`git`·`docker` CLI뿐이며 모두 타임아웃 + 실패 격리로 실행됩니다.

## 알려진 제약

- 다른 사용자 소유 프로세스는 표시만 되고 종료할 수 없습니다 (macOS 권한).
- Start는 감지된 패키지 매니저 스크립트(`dev`, `start:dev`, `serve`, `start`)와 Docker Compose만 지원 — 임의 명령은 프로젝트별 override로 지정.
- Docker 기능은 `docker` CLI가 PATH에 있어야 합니다.
- 서명/공증되지 않은 빌드라 첫 실행 시 Gatekeeper 확인이 필요할 수 있고, 릴리즈 빌드 알림은 최초 1회 권한을 요청합니다.

## 라이선스

[MIT](LICENSE)
