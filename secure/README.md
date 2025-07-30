# Delong Protocol - Secure Service

Secure服务是Delong Protocol在TEE（可信执行环境）中运行的核心后端服务，负责处理算法执行、数据加密和链上交互等敏感业务逻辑。

## 目录

- [功能特性](#功能特性)
- [系统要求](#系统要求)
- [开发环境设置](#开发环境设置)
- [配置说明](#配置说明)
- [运行服务](#运行服务)
- [测试](#测试)
- [项目结构](#项目结构)
- [API文档](#api文档)
- [故障排除](#故障排除)

## 功能特性

- **算法执行引擎**: 在隔离环境中安全执行用户算法
- **数据加密存储**: 使用TEE保护敏感数据
- **链上交互**: 与智能合约进行安全交互
- **IPFS集成**: 分布式存储算法和数据
- **任务调度**: 高效的任务队列和并发执行
- **状态同步**: 与区块链状态保持同步

## 系统要求

- Rust 1.70+ (推荐使用最新稳定版)
- Docker 20.10+
- Docker Compose 2.0+
- PostgreSQL 16+ (通过Docker提供)
- Redis 7+ (通过Docker提供)
- Python 3.8+ (用于算法执行)

## 开发环境设置

### 1. 克隆仓库

```bash
git clone <repository-url>
cd delong-protocol/secure
```

### 2. 安装Rust工具链

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 3. 启动开发环境

我们提供了一个便捷的脚本来启动所有必需的服务：

```bash
./scripts/start-dev.sh
```

这个脚本会：
- 启动PostgreSQL数据库
- 启动Redis缓存服务
- 启动Anvil（本地以太坊节点）
- 启动IPFS节点
- 运行数据库迁移
- 加载环境变量

### 4. 手动启动服务

如果您想手动控制服务，可以使用Docker Compose：

```bash
# 启动所有服务
docker-compose -f docker-compose.dev.yml up -d

# 查看服务日志
docker-compose -f docker-compose.dev.yml logs -f

# 停止所有服务
docker-compose -f docker-compose.dev.yml down
```

## 配置说明

### 环境变量

复制 `.env.dev` 文件并根据需要修改：

```bash
cp .env.dev .env
```

主要配置项：

| 变量名 | 说明 | 默认值 |
|--------|------|--------|
| `SERVER_PORT` | 服务端口 | 8090 |
| `DATABASE_URL` | PostgreSQL连接URL | postgres://secure_user:secure_dev_password@localhost:5432/secure_db |
| `CHAIN_RPC_URL` | 以太坊RPC端点 | http://localhost:8545 |
| `IPFS_API_URL` | IPFS API端点 | http://localhost:5001 |
| `TEE_ENABLED` | 是否启用TEE | false |

### 开发环境服务端口

- **PostgreSQL**: 5432
- **Redis**: 6379
- **Anvil (Ethereum)**: 8545 (HTTP & WebSocket)
- **IPFS API**: 5001
- **IPFS Gateway**: 8080
- **Secure Service**: 8090

## 运行服务

### 开发模式

```bash
# 使用开发环境配置运行
cargo run

# 启用详细日志
RUST_LOG=debug cargo run

# 监听文件变化自动重启
cargo install cargo-watch
cargo watch -x run
```

### 生产构建

```bash
# 构建优化版本
cargo build --release

# 运行生产版本
./target/release/secure
```

## 测试

### 运行所有测试

```bash
cargo test
```

### 运行特定测试

```bash
# 运行单元测试
cargo test --lib

# 运行集成测试
cargo test --test '*'

# 运行特定模块的测试
cargo test execution::tests
```

### 测试覆盖率

```bash
# 安装tarpaulin
cargo install cargo-tarpaulin

# 生成覆盖率报告
cargo tarpaulin --out Html
```

## 项目结构

```
secure/
├── src/
│   ├── main.rs              # 应用入口
│   ├── config.rs            # 配置管理
│   ├── server.rs            # HTTP服务器
│   ├── state.rs             # 应用状态管理
│   ├── api/                 # API处理器
│   │   ├── mod.rs
│   │   ├── execution.rs     # 算法执行API
│   │   ├── storage.rs       # 存储API
│   │   └── chain.rs         # 链交互API
│   ├── domain/              # 领域模型
│   │   ├── mod.rs
│   │   ├── algorithm.rs
│   │   ├── execution.rs
│   │   └── dataset.rs
│   ├── infra/               # 基础设施
│   │   ├── db/              # 数据库
│   │   ├── ipfs/            # IPFS客户端
│   │   ├── chain/           # 区块链客户端
│   │   └── tee/             # TEE集成
│   ├── services/            # 业务服务
│   │   ├── execution.rs     # 执行服务
│   │   ├── storage.rs       # 存储服务
│   │   └── scheduler.rs     # 调度服务
│   └── workers/             # 后台任务
│       ├── mod.rs
│       ├── chain_sync.rs    # 链同步
│       └── execution.rs     # 执行工作器
├── migrations/              # 数据库迁移
├── scripts/                 # 开发脚本
├── tests/                   # 集成测试
└── docker-compose.dev.yml   # 开发环境配置
```

## API文档

启动服务后，可以访问以下端点查看API文档：

- Swagger UI: http://localhost:8090/swagger-ui
- OpenAPI规范: http://localhost:8090/api-doc/openapi.json

### 主要API端点

- `POST /api/v1/algorithms` - 创建算法
- `POST /api/v1/executions` - 创建执行任务
- `GET /api/v1/executions/{id}` - 查询执行状态
- `GET /api/v1/executions/{id}/result` - 获取执行结果

## 故障排除

### 常见问题

1. **数据库连接失败**
   ```bash
   # 检查PostgreSQL是否运行
   docker ps | grep postgres
   
   # 查看数据库日志
   docker logs secure-postgres-dev
   ```

2. **端口冲突**
   ```bash
   # 检查端口占用
   lsof -i :5432  # PostgreSQL
   lsof -i :8545  # Anvil
   ```

3. **IPFS连接问题**
   ```bash
   # 检查IPFS状态
   curl http://localhost:5001/api/v0/version
   ```

4. **编译错误**
   ```bash
   # 清理并重新构建
   cargo clean
   cargo build
   ```

### 日志调试

使用环境变量控制日志级别：

```bash
# 查看所有日志
RUST_LOG=trace cargo run

# 只查看secure模块的日志
RUST_LOG=secure=debug cargo run

# 查看特定模块的日志
RUST_LOG=secure::execution=trace cargo run
```

## 贡献指南

请参考项目根目录的 [CONTRIBUTING.md](../../CONTRIBUTING.md) 文件。

## 许可证

本项目采用 [MIT License](../../LICENSE) 许可证。