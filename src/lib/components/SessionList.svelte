<script lang="ts">
    import type { Session, ProjectStatus } from "$lib/types";
    import { _ } from "svelte-i18n";

    interface Props {
        sessions: Session[];
        activeSessionId: string | undefined;
        onSelect: (id: string) => void;
        onDelete: (id: string) => void;
        onCreate: () => void;
    }

    let { sessions, activeSessionId, onSelect, onDelete, onCreate }: Props =
        $props();

    // Simple colored dot for status
    const statusColors: Record<ProjectStatus, string> = {
        brainstorming: "#a855f7", // purple
        ready: "#6e6e6e", // gray
        queued: "#3794ff", // blue
        running: "#4ec9b0", // green
        pausing: "#cca700", // yellow
        paused: "#cca700", // yellow
        done: "#4ec9b0", // green
        partial: "#3794ff", // blue
        failed: "#f14c4c", // red
        cancelled: "#6e6e6e", // gray
    };

    const animatedStatuses: ProjectStatus[] = ["running", "pausing"];

    function getStatusColor(status: ProjectStatus) {
        return statusColors[status] || statusColors.ready;
    }

    function shouldAnimate(status: ProjectStatus) {
        return animatedStatuses.includes(status);
    }
</script>

<div class="flex flex-col h-full bg-vscode-sidebar">
    <div
        class="px-4 py-2 text-xs font-semibold text-vscode-muted uppercase flex justify-between items-center"
    >
        <span>{$_("common.sessions") || "Sessions"}</span>
        <button
            class="p-1 hover:bg-vscode-hover rounded text-vscode-accent cursor-pointer"
            onclick={onCreate}
            title="New Session"
        >
            <svg
                class="w-3.5 h-3.5"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
                stroke-width="2"
            >
                <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    d="M12 4v16m8-8H4"
                />
            </svg>
        </button>
    </div>

    <div class="flex-1 overflow-y-auto">
        {#if !sessions || sessions.length === 0}
            <div class="px-4 py-2 text-center text-vscode-muted text-xs italic">
                No sessions
            </div>
        {:else}
            {#each sessions as session (session.id)}
                <div
                    class="w-full px-4 py-1.5 text-left hover:bg-vscode-hover transition-colors group cursor-pointer flex items-center gap-2
            {activeSessionId === session.id
                        ? 'bg-vscode-active text-white'
                        : 'text-vscode'}"
                    onclick={() => onSelect(session.id)}
                    onkeydown={(e) => e.key === "Enter" && onSelect(session.id)}
                    role="button"
                    tabindex="0"
                >
                    <!-- Simple status dot -->
                    <div
                        class="w-1.5 h-1.5 rounded-full flex-shrink-0 {shouldAnimate(
                            session.status,
                        )
                            ? 'animate-pulse'
                            : ''}"
                        style="background-color: {getStatusColor(
                            session.status,
                        )}"
                    ></div>
                    <div class="flex-1 min-w-0" title={session.name}>
                        <div class="text-xs truncate">
                            {session.name}
                        </div>
                    </div>
                    <button
                        class="opacity-0 group-hover:opacity-100 p-0.5 hover:bg-[#f14c4c20] rounded text-[#f14c4c]"
                        onclick={(e) => {
                            e.stopPropagation();
                            onDelete(session.id);
                        }}
                        title={$_("common.delete")}
                    >
                        <svg
                            class="w-3 h-3"
                            fill="none"
                            stroke="currentColor"
                            viewBox="0 0 24 24"
                            stroke-width="2"
                        >
                            <path
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                d="M6 18L18 6M6 6l12 12"
                            />
                        </svg>
                    </button>
                </div>
            {/each}
        {/if}
    </div>
</div>
