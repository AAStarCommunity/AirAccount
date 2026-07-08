# AAStar 社区节点下载门户(Phase 3 脚手架)

社区上手程序 Phase 3 的下载门户(见 `../docs/community-onboarding-program.md`)。

## 是什么
`index.html` —— 一个自包含静态页(内联 CSS,theme-aware),把社区上手串起来:
三步走 → 三档傻瓜度(预刷板 / 镜像 / installer)→ 下载发布物 → 向导会问什么 → 文档。

## 部署
纯静态,任意托管:
- **GitHub Pages**:把 `kms/portal/` 设为 Pages 源,或 copy 到 `docs/`。
- **Cloudflare Pages / 任意静态托管**:上传 `index.html`。
- 本地预览:`python3 -m http.server -d kms/portal 8000`。

## 脚手架边界(Phase 3 待接线)
- **下载链接**:现指向 GitHub Releases tag 页;`.wic` 镜像(Phase 2 路径 B)待 CI 出。
- **gasless 一键注册闭环**:向导内直接提交给 AAStar owner 注册(现为文字指引)—— 依赖 AAStar owner-side gasless 注册服务。
- **动态化**:releases 列表现为硬编码,可后续拉 GitHub API 动态渲染。
- **节点健康/监控回传**:未做。
