# 사용 가이드

Dev Cockpit을 설치하고 매일 사용하는 데 필요한 모든 것.

*English version: [USAGE.md](USAGE.md)*

## 설치

### npm으로 설치 (Apple Silicon)

```bash
npx @min0504/dev-cockpit
```

최신 릴리즈를 내려받아 SHA-256 체크섬을 검증하고 `Dev Cockpit.app`을 `/Applications`(쓰기 불가 시 `~/Applications`)에 설치한 뒤 실행합니다. 옵션: `--dir <경로>`로 설치 위치 지정, `--no-open`으로 실행 생략.

### 소스에서 빌드

```bash
git clone https://github.com/Min0504/dev-cockpit.git
cd dev-cockpit
npm install
npm run tauri build
```

번들 생성 위치:

```
src-tauri/target/release/bundle/macos/Dev Cockpit.app
```

`/Applications`로 옮긴 뒤 실행하세요.

### 첫 실행

- **Gatekeeper** — 코드서명이 없는 앱입니다. macOS가 차단하면 앱을 우클릭 → 열기 → 열기.
- **알림 권한** — 릴리즈 빌드는 최초 1회 알림 권한을 요청합니다. 서버 다운·헬스 실패·포트 충돌 알림을 받으려면 허용하세요.
- **Launch at Login** — 트레이 메뉴나 설정에서 켜두면 항상 사용할 수 있습니다.

실행하면 메뉴바에 게이지 아이콘이 나타납니다. 서비스가 돌고 있으면 아이콘 옆에 개수(실행 중 서비스 + 컨테이너)가 표시되고, 마우스를 올리면 툴팁으로 상세 내역을 보여줍니다.

## 패널

트레이 아이콘 클릭(또는 어디서든 `⌃⌥D`)으로 패널을 엽니다. 트레이 아이콘 아래에 붙어서 열리고, 포커스를 잃으면 자동으로 숨습니다 — 핀을 켜지 않았다면요.

위에서부터:

1. **검색창** — 입력 즉시 전체 필터링.
2. **충돌 배너** — 같은 포트에 바인드가 겹치는 리스너가 있을 때만 나타납니다. 각 프로세스·PID·프로젝트를 명시하고, 항목마다 stop 버튼이 있습니다.
3. **프로젝트 카드** — 무언가 실행 중인(또는 직접 펼친) 프로젝트마다 하나. 헤더: 프로젝트명, Git 브랜치, dirty 개수, ahead/behind 화살표, 마지막 커밋. 내부 행: 실행 중 서비스 → 컨테이너 → 시작 가능한(유휴) 서비스 순.
4. **Other dev processes** — 어느 프로젝트에도 연결하지 못한 개발성 프로세스.
5. **Docker** — 프로젝트에 연결되지 않은 컨테이너와 데몬 상태.
6. **Idle projects** — 아무것도 안 돌고 있는 프로젝트들의 접힌 그룹.
7. **상태바** — 마지막 스캔 시각과 소요 시간, 합계, 일시정지·즉시 스캔 버튼.

### 키보드

| 키 | 동작 |
| --- | --- |
| `⌃⌥D` (전역) | 어디서든 패널 토글 |
| `/` 또는 `⌘F` | 검색창 포커스 |
| `Esc` | 시트 닫기 → 검색 지우기 → 패널 숨기기 (순서대로) |

## 서비스

각 서비스 행: 상태 점, 프레임워크 배지, 이름, 포트 배지, CPU, 메모리, 업타임. 행에 마우스를 올리면 액션 버튼이 나타납니다.

- **상태 점** — 초록: 실행 중/정상 · 노랑: 이상 징후(TCP는 열렸는데 HTTP 실패, 컨테이너 health `starting`) · 빨강: 다운/unhealthy · 빈 원: 미실행.
- **포트 배지** — HTTP를 말하는 서비스면 클릭 시 `http://localhost:<port>`가 열립니다. 툴팁에 전체 포트 목록.
- **Stop** — SIGTERM 전송. `⌥`클릭은 SIGKILL. 대상이 자기 프로세스 그룹의 리더라면(예: `npm run dev` 트리) 그룹 전체를 종료해 고아 프로세스가 남지 않습니다. 감지된 개발 서비스만 종료할 수 있고 시스템 프로세스는 보호됩니다.
- **Start** — 시작 명령이 감지된 유휴 서비스에 나타납니다. 프로젝트의 패키지 매니저(lockfile 기준 `pnpm` / `yarn` / `bun` / `npm`)와 스크립트에서 `dev` → `start:dev` → `serve` → `start` 순으로 선택합니다. Compose 프로젝트는 `docker compose up -d`로 시작. 시작하면 실시간 로그 화면이 열립니다.
- **시작 명령 편집** — 유휴 서비스에 마우스를 올려 Start가 실행할 명령을 서비스별로 덮어쓸 수 있습니다. 프로젝트 단위로 저장됩니다.
- **Restart** — 중지 후 재시작. 시작 명령을 아는 경우에만 표시됩니다(앱에서 시작한 서비스는 항상 알고 있음).
- **로그** — Dev Cockpit에서 시작한 서비스는 stdout/stderr를 실시간으로 스트리밍합니다.

## 프로젝트

프로젝트 카드는 하나의 저장소/앱 디렉터리에 속한 모든 것을 묶습니다.

- **헤더 액션** (hover 시): compose up/down, 브라우저 열기, 터미널 열기, 에디터 열기, Finder에서 보기, 프로젝트 숨기기.
- **Git 정보** — 브랜치, 미커밋 변경 개수, upstream 대비 ahead/behind, 마지막 커밋 요약과 시점. 기본 20초마다 갱신(설정 가능).
- **디스커버리** — 설정된 루트 폴더에서 Git 저장소와 매니페스트(`package.json`, `pyproject.toml`, `docker-compose.yml` / `compose.yaml`, `go.mod`, `Cargo.toml`, `Gemfile`, `mix.exs`, …)를 찾아 프로젝트로 인식합니다. 모노레포 워크스페이스(`pnpm-workspace.yaml`, `package.json`의 `workspaces`)를 분석해 서브 패키지별로 시작 가능한 서비스를 만듭니다.
- **연결(attribution)** — 실행 중인 프로세스는 작업 디렉터리(부모 프로세스 포함)로 프로젝트에 연결됩니다. 하위 폴더에서 `pnpm dev`를 실행해도 올바른 카드에 표시됩니다.

## Docker

프로젝트에 연결된 컨테이너(compose 작업 디렉터리·라벨 기준)는 프로젝트 카드 안에, 나머지는 Docker 섹션에 표시됩니다.

- 행에는 컨테이너/compose 서비스명, 이미지, 호스트 포트, CPU, 메모리, 업타임이 나옵니다.
- 액션: 로그(실시간 `docker logs -f` tail), restart, stop / start.
- Compose 프로젝트는 카드에 **compose up / compose down** 버튼 하나로 제어하고, 출력은 로그 화면으로 스트리밍됩니다.
- Docker 데몬이 꺼져 있으면 Docker 기능만 조용히 비활성화됩니다 — 작은 안내만 표시되고 나머지는 전부 정상 동작합니다.

## 로그 화면

서비스/컨테이너의 로그 버튼으로 열거나, 무언가를 시작하면 자동으로 열립니다.

- **실시간 스트림** — 최근 2,000줄 유지(줄당 4,000자 제한)로 메모리가 항상 제한됩니다.
- **필터** — 입력한 문자열이 포함된 줄만 표시, 매치 개수 표시.
- **일시정지** — 화면을 멈추고 들어오는 줄은 버퍼링; 재개하면 따라잡습니다.
- **클리어** — 화면 비우기.
- **Follow** — 화면이 바닥에 붙어 따라갑니다. 위로 스크롤하면 해제, Follow 버튼으로 다시 붙입니다.
- 화면을 떠나면 로그 세션이 정리됩니다(컨테이너 tail 프로세스 종료). 백그라운드에 아무것도 남지 않습니다.

## 알림

발송 조건: 서비스 예기치 않은 종료 · 컨테이너 중지 · 헬스체크 실패(연속 2회) · 복구 · 새 포트 충돌 · Docker 데몬 끊김/복구.

- 이벤트별 쿨다운(기본 60초, 10초~1시간 설정 가능)으로 반복을 막습니다.
- 앱 시작 직후와 슬립/웨이크 직후의 스캔은 grace 기간 — 가짜 "종료" 알림이 없습니다.
- 직접 중지한 것은 알림이 오지 않습니다.
- 모든 이벤트를 설정에서 개별 on/off 할 수 있습니다.

## 설정

패널 헤더의 톱니 버튼으로 엽니다.

| 설정 | 의미 | 기본값 |
| --- | --- | --- |
| Theme | 시스템 / 라이트 / 다크 | 시스템 |
| Launch at Login | macOS 로그인 항목 | 꺼짐 |
| Keep window open | 블러 시 숨김 해제(핀) | 꺼짐 |
| Scan interval | 포트+프로세스 스캔 주기 | 3초 (2/3/5/10) |
| Docker interval | `docker ps` 주기 | 5초 |
| HTTP health checks | 로컬 HTTP 포트 프로브 | 켜짐 |
| Show idle projects | 미실행 프로젝트 표시 | 켜짐 |
| Show other listeners | 개발 외 프로세스의 열린 포트 표시 | 꺼짐 |
| Notifications | 마스터 + 이벤트별 토글 + 쿨다운 | 켜짐, 60초 |
| Project roots | 프로젝트를 찾을 폴더 | `~/Dev`, `~/Developer`, `~/Projects`, `~/Code`, `~/repos`, `~/workspace` 중 존재하는 것 |
| Projects | 이름 변경, 숨기기/해제, 프로젝트별 override | — |

### 설정 파일

모든 설정은 JSON 파일 하나에 저장되며 직접 편집하거나 백업할 수 있습니다:

```
~/Library/Application Support/com.min0504.devcockpit/config.json
```

앱은 자신이 쓴 내용만 반영합니다. 파일을 손으로 고쳤다면 앱을 재시작하세요.

## 트레이 메뉴

트레이 아이콘 우클릭(또는 클릭):

- **Open Dev Cockpit** (`⌃⌥D`)
- **Rescan Now** — 즉시 전체 스캔 + 프로젝트 재발견
- **Pause Monitoring** — 모든 스캔 중지 (체크박스)
- **Launch at Login** (체크박스)
- **Quit**

## 제거

1. Dev Cockpit 종료 (트레이 메뉴 → Quit).
2. `Dev Cockpit.app` 삭제.
3. 원하면 설정도 삭제: `~/Library/Application Support/com.min0504.devcockpit/`.

시스템 다른 곳에는 어떤 파일도 만들지 않습니다.
