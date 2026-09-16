# Project instructions

## Desktop design

Before designing, implementing, or reviewing any desktop UI, read and follow
[course2md-design](.agents/skills/course2md-design/SKILL.md).
It is the current design authority, including when older mockups or design documents disagree.
Keep colors, controls, icons and motion in the shared design primitives; do not create a new visual dialect in a page.

For a whole-product UI change, use subagents to independently review every affected page and its loading, empty, failure and completion states. Record concrete findings and their resolution. Verify the running native application, not just source or HTML mockups.

The user has excluded specialist accessibility audits from this redesign. Preserve ordinary keyboard behavior and existing system preferences without expanding the task into an accessibility project.

Keep changes reviewable and make atomic Git commits by concern. Do not change release tags or replace published assets as part of an ordinary UI change.

## 分支与上游同步

- `main` 是本 fork 的基线，跟踪 `origin/main`；不要把未审查的上游提交或功能开发直接写入这里。
- `sync/upstream-main` 只跟踪原作者仓库 `upstream/main`，保持可快进更新，不在此分支提交本地功能。
- `feature/windows-desktop-setup` 是本项目的功能集成分支，当前 Windows 关闭、uv、依赖目录和首次配置改动都在这里；后续 agents 默认在此分支工作。
- 原作者在 `sync/upstream-main` 中新增的 UI、动效、滚动条或其他行为，必须先比较影响，再合并或 cherry-pick 到 `feature/windows-desktop-setup`，不能因为分支分离而遗漏。

同步流程：先在 `sync/upstream-main` 执行 `git pull --ff-only`，再切回功能分支，审查后执行 `git merge --no-ff sync/upstream-main` 或按提交 cherry-pick，最后运行受影响测试。保持 `sync/upstream-main` 不含本地提交。
