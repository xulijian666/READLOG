# CGIS SIT 日志存储结构分析

## 基本信息

- **访问地址**: `http://10.142.149.25:61000/fileviewer/gcis/SIT/log/gemini/SIT/`
- **认证方式**: HTTP Basic Auth (`cgisteam` / `Aa123456!`)
- **环境**: SIT (系统集成测试)

---

## 整体目录结构

```
SIT/
├── C1/                          # 集群节点 1 (24 个服务目录，最完整)
├── C2/                          # 集群节点 2 (18 个服务目录)
└── C3/                          # 集群节点 3 (19 个服务目录)
```

**C1/C2/C3** 代表 Kubernetes 集群中的不同 Pod 或节点。同一服务在不同节点上会各自产生独立的日志。

---

## 服务目录清单

| 服务名称 | 类型 | C1 | C2 | C3 |
|---|---|---|---|---|
| `cgis-front-gemini` | 前端网关 (OpenResty/Nginx) | ✅ | ✅ | ✅ |
| `cgis-web-gateway` | Spring Cloud Gateway | ✅ | ✅ | ✅ |
| `cgis-web-claim` | 理赔服务 | ✅ | ✅ | ✅ |
| `cgis-web-policy` | 保单服务 | ✅ | ✅ | ✅ |
| `cgis-web-prem` | 保费服务 | ✅ | ✅ | ✅ |
| `cgis-web-uw` | 核保服务 | ✅ | ✅ | ✅ |
| `cgis-web-nb` | 新契约服务 | ✅ | ✅ | ✅ |
| `cgis-web-nbx` | 新契约扩展服务 | ✅ | ✅ | ✅ |
| `cgis-web-benefit` | 给付服务 | ✅ | ✅ | ✅ |
| `cgis-web-foundation` | 基础服务 | ✅ | ✅ | ✅ |
| `cgis-web-interface` | 接口服务 | ✅ | ✅ | ✅ |
| `cgis-web-system` | 系统管理服务 | ✅ | ✅ | ✅ |
| `cgis-web-workflow` | 工作流服务 | ✅ | ✅ | ✅ |
| `cgis-web-report` | 报表服务 | ✅ | ✅ | ✅ |
| `cgis-web-ruleclm` | 理赔规则引擎 | ✅ | ✅ | ✅ |
| `cgis-web-ruleprem` | 保费规则引擎 | ✅ | ✅ | ✅ |
| `cgis-web-ruleuw` | 核保规则引擎 | ✅ | ✅ | ✅ |
| `cgis-web-bi` | BI 服务 | ✅ | - | - |
| `cgis-web-bi2` | BI 服务 (备) | ✅ | - | - |
| `cgis-web-template` | 模板服务 | ✅ | - | ✅ |
| `cgis-web-metabase` | Metabase 可视化 | ✅ | - | - |
| `cgis-infra-apiportal` | API Portal 网关 | ✅ | - | - |
| `cgis-tool-javaagent` | Java Agent 工具 | ✅ | - | - |
| `elastic-apm-agent` | Elastic APM Agent | ✅ | ✅ | ✅ |

---

## 日志文件命名规则

### 1. 前端网关日志 (`cgis-front-gemini`)

基于 OpenResty/Nginx，日志直接存储在服务目录下（无 IP 子目录）：

```
cgis-front-gemini/
├── access-2026-05-02T18.log      # 按小时切割的访问日志
├── access-2026-05-02T19.log
├── access-2026-05-06T22.log
└── error.log                      # Nginx 错误日志（累积）
```

- **命名格式**: `access-{YYYY-MM-DD}T{HH}.log`
- **切割频率**: 每小时一个文件
- **保留范围**: 约 4 天 (102 个文件)
- **C2 节点特殊**: 只有 `access.log` + `error.log`（滚动日志，不分割）
- **格式**: pipe 分隔的自定义格式（C1/C3）或 Nginx 标准 combined 格式（C2）

**C1/C3 访问日志格式** (pipe 分隔):
```
客户端IP | "-" | 时间戳 | "请求方法 URI 协议" | "状态码" | "上游状态" | 请求体大小 | 响应体大小 | upstream_time | request_time | "Referer" | "User-Agent" | "-" | 其他指标
```

**C2 访问日志格式** (Nginx combined):
```
客户端IP - - [时间] "请求" "状态码" 响应大小 "Referer" "UA" "-" upstream_time request_time 请求大小 处理时间
```

### 2. Java 微服务应用日志 (`cgis-web-*`)

日志按 **服务名 → IP 地址** 两级目录组织：

```
cgis-web-gateway/
└── 10.142.149.161/              # Pod IP 地址
    ├── app.2026-05-02-18.log.gz  # 按小时切割，gzip 压缩的历史日志
    ├── app.2026-05-02-19.log.gz
    ├── app.2026-05-06-18.log     # 当前小时未压缩
    ├── app.2026-05-06-19.log
    ├── app.2026-05-06-20.log
    ├── app.log                   # 当前活跃日志文件
    └── DLP/                      # 数据防泄漏审计日志 (通常为空)
```

- **命名格式**: `app.{YYYY-MM-DD}-{HH}.log[.gz]`
- **切割频率**: 每小时一个文件
- **压缩策略**: 历史小时日志自动 gzip 压缩，当前小时保持 `.log`
- **`app.log`**: 实时写入的活跃日志文件
- **保留范围**: 约 4 天
- **格式**: Spring Boot Logback 格式

**日志格式示例**:
```
2026-05-06 21:00:00.721 [线程名] [traceId-spanId-sampled] INFO  全限定类名 - 日志消息
```

### 3. Elastic APM Agent 日志 (`elastic-apm-agent`)

```
elastic-apm-agent/
└── 10.142.149.106/              # Pod IP
    └── cgis-web-prem/           # 对应的服务名
        └── elk-apm.log          # APM Agent 运行日志
```

- **目录结构**: IP → 服务名 → 日志文件
- **日志类型**: Elastic APM Java Agent 的运行时日志（性能追踪、链路追踪相关）

### 4. API Portal 日志 (`cgis-infra-apiportal`)

```
cgis-infra-apiportal/
├── 10.142.149.109/
│   └── DLP/                     # 审计日志目录 (通常为空)
└── 10.142.152.200/
    ├── app.log                  # Spring WebFlux 应用日志
    └── DLP/
```

- 与普通微服务相同的日志格式

### 5. Java Agent 工具 (`cgis-tool-javaagent`)

```
cgis-tool-javaagent/
└── jacoco/                      # 代码覆盖率采集 (通常为空)
```

---

## 节点差异总结

| 特征 | C1 | C2 | C3 |
|---|---|---|---|
| 服务数量 | 24 | 18 | 19 |
| front-gemini 日志切割 | 按小时 (102 个文件) | 单文件滚动 | 按小时 (102 个文件) |
| front-gemini 访问格式 | pipe 分隔 | Nginx combined | pipe 分隔 |
| front-gemini error.log 时间 | 2025/08 起 | 2025/11 起 | 类似 C1 |
| 额外服务 | bi, bi2, metabase, apiportal, javaagent | - | template |

---

## 日志存储规律总结

1. **路径模板**: `SIT/{C1,C2,C3}/{服务名}/{Pod IP}/{日志文件}`
2. **切割粒度**: 所有日志均按 **小时** 切割（`YYYY-MM-DD-HH` 或 `YYYY-MM-DDTHH`）
3. **压缩策略**: 历史日志 gzip 压缩（`.log.gz`），当前小时不压缩，实时写入 `app.log`
4. **保留周期**: 约 4 天（~100 个按小时切割的文件）
5. **front-gemini 特殊**: 作为 Nginx/OpenResty 前端代理，日志直接在服务目录下，不按 IP 分目录
6. **DLP 目录**: 每个后端服务下都有 `DLP/` 子目录（数据防泄漏审计），但目前均为空
7. **服务本质**: 所有 `cgis-web-*` 均为 **Spring Boot** 应用（通过 `lettuce`、`reactor-http-epoll` 等线程名可判断），使用 **Logback** 日志框架
8. **前端代理**: `cgis-front-gemini` 基于 **OpenResty** (Nginx + Lua)，负责反向代理和静态资源服务

---

## 日志内容示例

### Spring Boot 应用日志
```
2026-05-06 21:00:00.721 [lettuce-epollEventLoop-4-4] [cgis-web-gateway-38d9c2d3...] INFO  c.c.c.c.s.CgisReactiveWebSecurityConfig - 用户[User(userId=009, ...)] 已授权访问 INTERFACE/servlet/GCISUIXDispatcher/InstantUIX/QUERYLETTER
```

### Nginx 访问日志 (C1/C3 格式)
```
10.142.149.25|"-"|2026-05-06T21:00:00+08:00|"POST /INTERFACE/servlet/GCISUIXDispatcher/InstantUIX/QUERYLETTER HTTP/1.1"|"200"|"200"|1045|513|0.059|0.059|"http://..."|"Mozilla/5.0 ..."|"-"|4.47|693
```

### Nginx 错误日志
```
2025/08/19 14:58:53 [error] 9#9: *9 open() "/usr/local/openresty/nginx/statics/health.html" failed (2: No such file or directory), client: 10.142.152.89, server: localhost, request: "GET /health.html HTTP/1.1"
```

---

## C1 节点系统数据流转

### 架构总览

```
用户浏览器 (Chrome)
    │
    ▼
┌─────────────────────────────────┐
│  cgis-front-gemini              │  OpenResty/Nginx (端口 61001)
│  反向代理 + 静态资源服务         │  Pod IP: 10.142.149.25
└──────────┬──────────────────────┘
           │ HTTP (反向代理)
           ▼
┌─────────────────────────────────┐
│  cgis-web-gateway               │  Spring Cloud Gateway (端口 8080)
│  认证鉴权 + XSS过滤 + 路由分发   │  Pod IP: 10.142.149.161
│  技术栈: Spring WebFlux         │
│  Session: Redis (Lettuce 客户端) │
│  ORM: MyBatis                   │
└──────────┬──────────────────────┘
           │
    ┌──────┼──────┬──────┬──────┬──────┐
    ▼      ▼      ▼      ▼      ▼      ▼
 cgis-web cgis-web cgis-web cgis-web cgis-web  ...
 -claim   -policy  -prem   -nb     -uw
 (理赔)   (保单)   (保费)   (新契约) (核保)
```

### 请求流转过程 (以 QUERYLETTER 为例)

1. **用户浏览器** → `http://10.142.149.25:61001/pages/cgisMain.html` (OpenResty 静态页面)
2. 浏览器发起 `POST /INTERFACE/servlet/GCISUIXDispatcher/InstantUIX/QUERYLETTER`
3. **cgis-front-gemini** (OpenResty) 记录 access log，将请求反向代理到后端
4. **cgis-web-gateway** 收到请求:
   - `CgisReactiveWebSecurityConfig`: 从 Redis session 中读取用户信息，校验会话有效性
   - 查询 `LDMenu` + `LDMenuGrpToMenu` 表: `select distinct b.menugrpcode from ldmenu a left join ldmenugrptomenu b on a.nodecode = b.nodecode where a.runscript = ?`
   - 根据 URL 脚本路径匹配对应角色权限（如 `J-HBCZG`、`J-KHJLTDZ`）
   - `XSSFilter`: 检查 URL 是否需要 XSS 过滤
   - `LDUSERTRACE` 表记录用户操作审计: 插入操作人、操作内容、客户端 IP、时间等
5. Gateway 路由到对应的后端微服务处理业务逻辑
6. 响应原路返回

### 技术组件日志特征

| 组件 | 日志特征关键词 | 出现位置 |
|---|---|---|
| Spring Security | `CgisReactiveWebSecurityConfig` | cgis-web-gateway |
| MyBatis SQL | `==> Preparing:` / `==>` Parameters` / `<== Total` | 所有 cgis-web-* |
| Redis Session | `springSessionRedisMessageListenerContainer` / `Lettuce` | cgis-web-gateway, cgis-web-system |
| XSS Filter | `XSSFilter` / `NotCheckUrl` | cgis-web-gateway |
| 用户审计 | `LDUSERTRACE` / `catalogWatchTaskScheduler` | cgis-web-gateway |
| 权限校验 | `selectGrpCodeByMenuScript` / `LDMenu` / `LDMenuGrpToMenu` | cgis-web-gateway |
| Elastic APM | `elastic-apm-server-reporter` / `co.elastic.apm.agent` | elastic-apm-agent |
| OpenResty | `[error] 9#9:` / Nginx worker 编号 | cgis-front-gemini error.log |
| WebFlux | `reactor-http-epoll` 线程名 | cgis-infra-apiportal, cgis-web-gateway |

### 链路追踪

- **Trace ID 格式**: `cgis-web-{service}-{traceId}-{spanId}-{sampled}`
  - 示例: `cgis-web-gateway-38d9c2d321d3508b-38d9c2d321d3508b-false`
- **Elastic APM**: 已集成 Java Agent，目标 APM Server 为 `http://192.168.140.77:9200`，但当前连接超时（APM 数据上报失败，仅保留 Agent 自身日志）

### 关键 IP 地址

| IP | 角色 |
|---|---|
| `10.142.149.25` | front-gemini (OpenResty 入口) |
| `10.142.149.161` | cgis-web-gateway (C1) |
| `10.142.152.28` | Kubernetes Ingress 健康检查探针 (Go client) |
| `10.142.149.25` | 用户操作客户端 IP (通过 OpenResty 代理) |
| `192.168.140.77` | Elastic APM Server (不可达) |

---

## cgis-front-gemini Access Log 数据流转分析

### C1/C3 访问日志字段解析 (pipe 分隔)

```
字段1: 客户端 IP
字段2: "-" (ident，未启用)
字段3: 请求时间戳 (ISO 8601，含时区)
字段4: 请求行 (方法 + URI + 协议)
字段5: 最终响应状态码
字段6: 上游(upstream)响应状态码
字段7: 请求体大小 (bytes)
字段8: 响应体大小 (bytes)
字段9: upstream 响应时间 (秒)
字段10: 请求总处理时间 (秒)
字段11: Referer
字段12: User-Agent
字段13: "-" (额外字段)
字段14: 其他指标 (如连接时间)
字段15: 其他指标 (如处理大小)
```

### C2 访问日志字段解析 (Nginx combined 扩展)

```
客户端IP - - [时间] "请求行" "状态码" 响应大小 "Referer" "UA" "-" upstream_time request_time 请求大小 处理时间
```

### 请求类型分布

从 access log 中识别到的典型请求模式:

| 请求路径 | 说明 | 来源 |
|---|---|---|
| `/INTERFACE/servlet/GCISUIXDispatcher/InstantUIX/{业务代码}` | 核心业务请求（保全、理赔等 UIX 分发） | 用户浏览器 |
| `/INTERFACE/common/cvar/CExecJson.jsp` | 通用数据查询接口 | 用户浏览器 |
| `/INTERFACE/common/easyQueryVer3/EasyQueryJson.jsp` | EasyQuery 数据查询 | 用户浏览器 |
| `/nbx/application/*` | 新契约扩展模块（投保、核保等） | 用户浏览器 |
| `/nbx/policy/*` | 保单管理 | 用户浏览器 |
| `/nbx/workflow/*` | 工作流（任务池等） | 用户浏览器 |
| `/claim/*` | 理赔业务 | 用户浏览器 |
| `/core/sys/*` | 系统管理（字典、区域、机构查询） | 用户浏览器 |
| `/browser/errorLogs` | 前端错误上报 | 用户浏览器 |
| `/health.html` | K8s 健康检查 | Go-http-client (Ingress) |
| `/v3/segments` | Elastic APM RUM 数据上报 | 浏览器 APM JS SDK |

### 数据流向图

```
用户浏览器
  │
  │  POST /INTERFACE/servlet/GCISUIXDispatcher/InstantUIX/QUERYLETTER
  │  Host: 10.142.149.25:61001
  │  Referer: http://10.142.149.25:61001/pages/cgisMain.html
  │
  ▼
┌──────────────────────────────┐
│ front-gemini (C1 Pod)        │  ← 记录 access-2026-05-06T21.log
│ 请求时间: 2026-05-06T21:00:00 │  ← 状态码 200, 耗时 59ms
│ upstream_time: 0.059s        │
└──────────┬───────────────────┘
           │
           ▼
┌──────────────────────────────┐
│ gateway (C1 Pod 10.142.149.161)│  ← 记录 app.2026-05-06-21.log
│ 1. CAS SSO 会话校验          │
│ 2. 查询 LDMenu 权限表        │  ← MyBatis DEBUG 日志
│ 3. XSSFilter 过滤            │
│ 4. 写入 LDUSERTRACE 审计表    │  ← catalogWatchTaskScheduler
│ 5. 路由到后端服务             │
└──────────┬───────────────────┘
           │
           ▼
┌──────────────────────────────┐
│ 后端业务服务 (cgis-web-*)     │  ← 记录 app.log (各自的 Pod IP)
│ 处理业务逻辑                  │
│ 查询业务数据库                │  ← MyBatis SQL 日志
│ 返回响应                      │
└──────────────────────────────┘
```

### Elastic APM RUM (前端监控)

浏览器端通过 `elastic-apm-rum.umd.min.js` 采集前端性能数据，上报到 `/v3/segments` 和 `/v3/se`，由 front-gemini 代理到后端 APM 基础设施。

### 常见异常模式

| 现象 | 日志位置 | 说明 |
|---|---|---|
| `health.html` 404 持续报错 | front-gemini error.log | K8s Ingress 健康检查探针访问 `/health.html`，但 OpenResty 未配置该静态文件 |
| `SessionDestroyedEvent` 发布失败 | cgis-web-system app.log | Redis Session 过期事件通知失败，Session 清理异常 |
| APM Server 连接超时 | elastic-apm-agent elk-apm.log | APM Agent 无法连接 `192.168.140.77:9200`，链路追踪数据丢失 |
| 路径穿越攻击告警 | cgis-infra-apiportal app.log | 扫描器探测 `../../../../windows/win.ini`，Spring WebFlux 拦截并告警 |
