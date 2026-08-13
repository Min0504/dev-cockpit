//! Framework / service detection heuristics.
//!
//! Two sources feed detection:
//!   1. A running process (name + full command line + cwd)
//!   2. Project files (package.json deps, pyproject, compose images, …)

/// Broad category used for health-check strategy and UI colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Frontend,
    Backend,
    Database,
    Infra,
    Runtime,
    Other,
}

#[derive(Debug, Clone)]
pub struct Detected {
    pub key: &'static str,
    pub label: &'static str,
    pub category: Category,
}

fn d(key: &'static str, label: &'static str, category: Category) -> Detected {
    Detected { key, label, category }
}

/// Detect a framework/service from a process name and its command line.
/// Order matters: more specific frameworks first, generic runtimes last.
pub fn from_process(name: &str, cmd: &str) -> Option<Detected> {
    let cmd_l = cmd.to_lowercase();
    let name_l = name.to_lowercase();
    let has = |pat: &str| cmd_l.contains(pat);

    // --- databases / caches (match by binary name) ---
    if name_l == "postgres" || name_l == "postmaster" {
        return Some(d("postgres", "PostgreSQL", Category::Database));
    }
    if name_l == "redis-server" {
        return Some(d("redis", "Redis", Category::Database));
    }
    if name_l == "mysqld" || name_l == "mariadbd" {
        return Some(d("mysql", "MySQL", Category::Database));
    }
    if name_l == "mongod" {
        return Some(d("mongodb", "MongoDB", Category::Database));
    }
    if name_l == "clickhouse" || name_l.starts_with("clickhouse-") {
        return Some(d("clickhouse", "ClickHouse", Category::Database));
    }
    if name_l == "memcached" {
        return Some(d("memcached", "Memcached", Category::Database));
    }

    // --- docker port forwarders ---
    if name_l == "docker-proxy"
        || name_l.starts_with("com.docker")
        || name_l == "vpnkit"
        || name_l.contains("orbstack helper")
    {
        return Some(d("docker", "Docker", Category::Infra));
    }

    // --- node ecosystem (command line based) ---
    if has("next-server") || has("next dev") || has("/.bin/next") || has("next start") {
        return Some(d("nextjs", "Next.js", Category::Frontend));
    }
    if has("node_modules/vite") || has("/.bin/vite") || cmd_l.ends_with(" vite") || has("vite.js")
        || has("vite dev") || has("vite --")
    {
        return Some(d("vite", "Vite", Category::Frontend));
    }
    if has("@nestjs") || has("nest start") || has("nest-cli") {
        return Some(d("nestjs", "NestJS", Category::Backend));
    }
    if has("react-scripts") {
        return Some(d("cra", "React (CRA)", Category::Frontend));
    }
    if has("remix vite:dev") || has("@remix-run") {
        return Some(d("remix", "Remix", Category::Frontend));
    }
    if has("astro dev") || has("/.bin/astro") {
        return Some(d("astro", "Astro", Category::Frontend));
    }
    if has("nuxt dev") || has("/.bin/nuxt") || has("nuxi") {
        return Some(d("nuxt", "Nuxt", Category::Frontend));
    }
    if has("svelte-kit") || has("@sveltejs") {
        return Some(d("sveltekit", "SvelteKit", Category::Frontend));
    }
    if has("ng serve") || has("@angular") {
        return Some(d("angular", "Angular", Category::Frontend));
    }
    if has("storybook") {
        return Some(d("storybook", "Storybook", Category::Frontend));
    }
    if has("expo start") || has("expo/cli") {
        return Some(d("expo", "Expo", Category::Frontend));
    }
    if has("webpack") {
        return Some(d("webpack", "Webpack Dev", Category::Frontend));
    }
    if has("tsx watch") || has("ts-node") || has("nodemon") {
        return Some(d("node-dev", "Node (watch)", Category::Backend));
    }

    // --- python ecosystem ---
    if has("uvicorn") {
        return Some(d("fastapi", "Uvicorn", Category::Backend));
    }
    if has("gunicorn") {
        return Some(d("gunicorn", "Gunicorn", Category::Backend));
    }
    if has("manage.py runserver") {
        return Some(d("django", "Django", Category::Backend));
    }
    if has("flask run") || has("flask --app") {
        return Some(d("flask", "Flask", Category::Backend));
    }
    if has("streamlit run") {
        return Some(d("streamlit", "Streamlit", Category::Backend));
    }
    if has("jupyter") {
        return Some(d("jupyter", "Jupyter", Category::Backend));
    }
    if has("mkdocs serve") {
        return Some(d("mkdocs", "MkDocs", Category::Backend));
    }

    // --- other ecosystems ---
    if has("rails server") || name_l == "puma" || has("puma ") {
        return Some(d("rails", "Rails", Category::Backend));
    }
    if has("php artisan serve") {
        return Some(d("laravel", "Laravel", Category::Backend));
    }
    if has("spring-boot") || has("bootrun") {
        return Some(d("spring", "Spring", Category::Backend));
    }
    if name_l == "nginx" {
        return Some(d("nginx", "nginx", Category::Infra));
    }
    if name_l == "caddy" {
        return Some(d("caddy", "Caddy", Category::Infra));
    }
    if name_l == "ollama" || has("ollama serve") {
        return Some(d("ollama", "Ollama", Category::Backend));
    }
    if name_l == "ssh" && (has(" -l ") || has(" -n ") || has("-t ")) {
        // port-forwarding ssh sessions show up as listeners
        return Some(d("ssh", "SSH tunnel", Category::Infra));
    }

    // --- generic runtimes (lowest priority) ---
    if name_l == "node" || name_l.starts_with("node (") {
        return Some(d("node", "Node.js", Category::Runtime));
    }
    if name_l == "bun" {
        return Some(d("bun", "Bun", Category::Runtime));
    }
    if name_l == "deno" {
        return Some(d("deno", "Deno", Category::Runtime));
    }
    if name_l.starts_with("python") {
        return Some(d("python", "Python", Category::Runtime));
    }
    if name_l == "ruby" {
        return Some(d("ruby", "Ruby", Category::Runtime));
    }
    if name_l == "java" {
        return Some(d("java", "Java", Category::Runtime));
    }
    if name_l == "dotnet" {
        return Some(d("dotnet", ".NET", Category::Runtime));
    }
    if name_l == "php" {
        return Some(d("php", "PHP", Category::Runtime));
    }
    None
}

/// Runtime name from a process binary name.
pub fn runtime_of(name: &str) -> Option<&'static str> {
    let n = name.to_lowercase();
    Some(match n.as_str() {
        "node" => "node",
        "bun" => "bun",
        "deno" => "deno",
        "ruby" => "ruby",
        "java" => "java",
        "dotnet" => "dotnet",
        "php" => "php",
        _ if n.starts_with("python") => "python",
        _ if n == "postgres" || n == "postmaster" => "postgres",
        _ if n == "redis-server" => "redis",
        _ if n == "mysqld" || n == "mariadbd" => "mysql",
        _ if n == "mongod" => "mongodb",
        _ => return None,
    })
}

/// Map a package.json dependency name to a framework.
pub fn from_node_dep(dep: &str) -> Option<Detected> {
    Some(match dep {
        "next" => d("nextjs", "Next.js", Category::Frontend),
        "vite" => d("vite", "Vite", Category::Frontend),
        "@nestjs/core" => d("nestjs", "NestJS", Category::Backend),
        "react-scripts" => d("cra", "React (CRA)", Category::Frontend),
        "@remix-run/react" | "@remix-run/node" => d("remix", "Remix", Category::Frontend),
        "astro" => d("astro", "Astro", Category::Frontend),
        "nuxt" => d("nuxt", "Nuxt", Category::Frontend),
        "@sveltejs/kit" => d("sveltekit", "SvelteKit", Category::Frontend),
        "@angular/core" => d("angular", "Angular", Category::Frontend),
        "express" => d("express", "Express", Category::Backend),
        "fastify" => d("fastify", "Fastify", Category::Backend),
        "hono" => d("hono", "Hono", Category::Backend),
        "koa" => d("koa", "Koa", Category::Backend),
        "electron" => d("electron", "Electron", Category::Other),
        "@tauri-apps/api" | "@tauri-apps/cli" => d("tauri", "Tauri", Category::Other),
        "react" => d("react", "React", Category::Frontend),
        "vue" => d("vue", "Vue", Category::Frontend),
        "svelte" => d("svelte", "Svelte", Category::Frontend),
        _ => return None,
    })
}

/// Priority when picking a single representative framework for a package:
/// meta-frameworks beat UI libraries.
pub fn node_dep_priority(key: &str) -> u8 {
    match key {
        "nextjs" | "nuxt" | "sveltekit" | "remix" | "astro" | "cra" | "angular" => 0,
        "nestjs" => 0,
        "vite" => 1,
        "express" | "fastify" | "hono" | "koa" => 2,
        "electron" | "tauri" => 3,
        "react" | "vue" | "svelte" => 4,
        _ => 5,
    }
}

/// Map a python dependency to a framework.
pub fn from_python_dep(dep: &str) -> Option<Detected> {
    let dl = dep.to_lowercase();
    Some(match dl.as_str() {
        "fastapi" => d("fastapi", "FastAPI", Category::Backend),
        "django" => d("django", "Django", Category::Backend),
        "flask" => d("flask", "Flask", Category::Backend),
        "streamlit" => d("streamlit", "Streamlit", Category::Backend),
        "uvicorn" => d("fastapi", "Uvicorn", Category::Backend),
        _ => return None,
    })
}

/// Map a docker image name to a service label, e.g. "postgres:16-alpine".
pub fn from_docker_image(image: &str) -> Option<Detected> {
    let base = image
        .split('/')
        .next_back()
        .unwrap_or(image)
        .split(':')
        .next()
        .unwrap_or(image)
        .to_lowercase();
    Some(match base.as_str() {
        "postgres" | "postgis" | "pgvector" => d("postgres", "PostgreSQL", Category::Database),
        "redis" | "valkey" => d("redis", "Redis", Category::Database),
        "mysql" | "mariadb" => d("mysql", "MySQL", Category::Database),
        "mongo" | "mongodb" => d("mongodb", "MongoDB", Category::Database),
        "rabbitmq" => d("rabbitmq", "RabbitMQ", Category::Infra),
        "kafka" | "cp-kafka" => d("kafka", "Kafka", Category::Infra),
        "elasticsearch" | "opensearch" => d("elasticsearch", "Elasticsearch", Category::Database),
        "clickhouse-server" | "clickhouse" => d("clickhouse", "ClickHouse", Category::Database),
        "minio" => d("minio", "MinIO", Category::Infra),
        "nginx" => d("nginx", "nginx", Category::Infra),
        "meilisearch" => d("meilisearch", "Meilisearch", Category::Database),
        "memcached" => d("memcached", "Memcached", Category::Database),
        "localstack" => d("localstack", "LocalStack", Category::Infra),
        _ => return None,
    })
}

/// Processes that listen on ports but are not development-related.
/// These are collapsed into the "Other" section of the UI.
pub fn is_noise(name: &str) -> bool {
    const NOISE: &[&str] = &[
        "rapportd",
        "sharingd",
        "controlce", // ControlCenter (lsof-truncated)
        "controlcenter",
        "identityservicesd",
        "airplayxpchelper",
        "remoted",
        "assistantd",
        "bluetoothd",
        "spotify",
        "dropbox",
        "onedrive",
        "creative cloud",
        "adobe",
        "gitkraken",
        "figma",
        "notion",
        "slack",
        "discord",
        "zoom.us",
        "microsoft teams",
        "steam",
        "logioptionsplus",
        "elgato",
        "raycast",
        "1password",
        "alfred",
        "cloudflared", // usually a background tunnel, not a project service
        "tailscaled",
        "mullvad",
        "expressvpn",
        "wireguard",
    ];
    let n = name.to_lowercase();
    if NOISE.iter().any(|p| n.contains(p)) {
        return true;
    }
    // Browsers & their helpers
    const BROWSERS: &[&str] = &["chrome", "safari", "firefox", "arc", "brave", "edge", "opera", "dia"];
    if BROWSERS.iter().any(|b| n == *b || n.starts_with(&format!("{b} ")) || n.contains("helper"))
        && !n.contains("node")
    {
        // "helper" alone is too broad; require browser-ish or Electron helper names
        if n.contains("helper") {
            const HELPER_OWNERS: &[&str] = &[
                "chrome", "safari", "firefox", "arc", "brave", "edge", "opera", "code", "cursor",
                "electron", "slack", "discord", "notion", "figma", "obsidian", "dia", "linear",
            ];
            return HELPER_OWNERS.iter().any(|o| n.contains(o));
        }
        return true;
    }
    false
}

/// Should this service get an HTTP health probe (vs TCP-only)?
pub fn is_http_category(cat: Category) -> bool {
    matches!(cat, Category::Frontend | Category::Backend | Category::Runtime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_common_dev_processes() {
        let cases = [
            ("node", "node /x/node_modules/.bin/vite --port 5173", "vite"),
            ("node", "next-server (v14.2.3)", "nextjs"),
            ("node", "node /p/node_modules/@nestjs/cli/bin/nest.js start --watch", "nestjs"),
            ("python3.12", "/usr/bin/python uvicorn main:app --reload", "fastapi"),
            ("python", "python manage.py runserver 0.0.0.0:8000", "django"),
            ("postgres", "/opt/homebrew/bin/postgres -D /opt/homebrew/var/postgresql@16", "postgres"),
            ("redis-server", "redis-server *:6379", "redis"),
            ("com.docker.backend", "/Applications/Docker.app/…", "docker"),
            ("node", "node server.js", "node"),
            ("bun", "bun run dev", "bun"),
        ];
        for (name, cmd, want) in cases {
            let got = from_process(name, cmd).map(|x| x.key).unwrap_or("none");
            assert_eq!(got, want, "name={name} cmd={cmd}");
        }
    }

    #[test]
    fn noise_filter() {
        assert!(is_noise("rapportd"));
        assert!(is_noise("Google Chrome Helper"));
        assert!(is_noise("ControlCe"));
        assert!(is_noise("Cursor Helper (Renderer)"));
        assert!(!is_noise("node"));
        assert!(!is_noise("postgres"));
        assert!(!is_noise("python3.12"));
    }

    #[test]
    fn docker_images() {
        assert_eq!(from_docker_image("postgres:16-alpine").unwrap().key, "postgres");
        assert_eq!(from_docker_image("redis:7-alpine").unwrap().key, "redis");
        assert!(from_docker_image("ghcr.io/foo/custom-api:latest").is_none());
    }

    #[test]
    fn node_dep_mapping() {
        assert_eq!(from_node_dep("next").unwrap().key, "nextjs");
        assert_eq!(from_node_dep("@nestjs/core").unwrap().key, "nestjs");
        assert!(from_node_dep("left-pad").is_none());
        assert!(node_dep_priority("nextjs") < node_dep_priority("react"));
    }
}
