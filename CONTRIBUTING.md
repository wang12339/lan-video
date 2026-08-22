# 贡献指南

感谢您对 Atmos Video 的贡献！

## 开发环境设置

1. Fork 并克隆仓库
2. 安装依赖
3. 配置环境变量
4. 运行开发服务器

## 代码规范

### Rust
- 使用 cargo fmt 格式化
- 使用 cargo clippy 检查
- 所有公共函数添加文档注释
- 错误处理使用 ServiceError

### TypeScript
- 使用 ESLint 检查
- 使用 Prettier 格式化
- 组件使用 React.memo 优化
- 使用 TypeScript 严格模式

## 提交规范

使用 Conventional Commits：
- feat: 新功能
- fix: Bug 修复
- docs: 文档更新
- refactor: 重构
- test: 测试
- chore: 其他

## PR 流程

1. 创建特性分支
2. 编写代码和测试
3. 确保 CI 通过
4. 请求代码审查
5. 合并到主分支

## Issue 模板

使用 Issue 模板报告 Bug 或提出功能请求。
