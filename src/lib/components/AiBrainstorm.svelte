<script lang="ts">
  import type { ProjectState, CliType } from '$lib/types';
  import * as api from '$lib/services/tauri';
  import type { ConversationMessage, AiBrainstormResponse } from '$lib/services/tauri';
  import { config } from '$lib/stores/settings';

  interface Props {
    project: ProjectState;
    onComplete: (project: ProjectState) => void;
    onCancel: () => void;
  }

  let { project, onComplete, onCancel }: Props = $props();

  let conversation = $state<ConversationMessage[]>([]);
  let userInput = $state('');
  let isLoading = $state(false);
  let isComplete = $state(false);
  let generatedPrompt = $state<string | null>(null);
  let selectedCli = $state<CliType>($config.defaultCli);
  let maxIterations = $state($config.defaultMaxIterations);
  let messagesContainer: HTMLDivElement;

  // Start with initial greeting
  $effect(() => {
    if (conversation.length === 0) {
      startConversation();
    }
  });

  async function startConversation() {
    isLoading = true;
    try {
      // Send empty conversation to get initial question
      const response = await api.aiBrainstormChat(project.id, [
        { role: 'user', content: '你好，我想开始一个新任务。' }
      ]);

      conversation = [
        { role: 'user', content: '你好，我想开始一个新任务。' },
        { role: 'assistant', content: response.message }
      ];

      if (response.isComplete && response.generatedPrompt) {
        isComplete = true;
        generatedPrompt = response.generatedPrompt;
      }
    } catch (error) {
      console.error('Failed to start brainstorm:', error);
      conversation = [
        { role: 'assistant', content: '你好！请告诉我你想要完成什么任务？我会帮你理清需求。' }
      ];
    } finally {
      isLoading = false;
    }
  }

  async function sendMessage() {
    if (!userInput.trim() || isLoading) return;

    const message = userInput.trim();
    userInput = '';

    // Add user message
    conversation = [...conversation, { role: 'user', content: message }];

    isLoading = true;
    try {
      const response = await api.aiBrainstormChat(project.id, conversation);

      // Add AI response
      conversation = [...conversation, { role: 'assistant', content: response.message }];

      if (response.isComplete && response.generatedPrompt) {
        isComplete = true;
        generatedPrompt = response.generatedPrompt;
      }

      // Scroll to bottom
      setTimeout(() => {
        if (messagesContainer) {
          messagesContainer.scrollTop = messagesContainer.scrollHeight;
        }
      }, 100);
    } catch (error) {
      console.error('Failed to send message:', error);
      conversation = [...conversation, {
        role: 'assistant',
        content: '抱歉，出现了错误。请重试或手动输入你的需求。'
      }];
    } finally {
      isLoading = false;
    }
  }

  async function handleComplete() {
    if (!generatedPrompt) return;

    isLoading = true;
    try {
      const updatedProject = await api.completeAiBrainstorm(
        project.id,
        generatedPrompt,
        selectedCli,
        maxIterations
      );
      onComplete(updatedProject);
    } catch (error) {
      console.error('Failed to complete brainstorm:', error);
    } finally {
      isLoading = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      sendMessage();
    }
  }
</script>

<div class="flex-1 flex flex-col h-full bg-gray-50 dark:bg-gray-900">
  <!-- Header -->
  <div class="p-4 bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
    <div>
      <h2 class="text-lg font-semibold text-gray-800 dark:text-white">
        AI Brainstorm - {project.name}
      </h2>
      <p class="text-sm text-gray-500 dark:text-gray-400">
        与 AI 对话，明确你的任务需求
      </p>
    </div>
    <button
      class="p-2 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
      onclick={onCancel}
    >
      ✕
    </button>
  </div>

  <!-- Chat Messages -->
  <div
    bind:this={messagesContainer}
    class="flex-1 overflow-y-auto p-4 space-y-4"
  >
    {#each conversation as message}
      <div class="flex {message.role === 'user' ? 'justify-end' : 'justify-start'}">
        <div class="max-w-[80%] p-3 rounded-lg {message.role === 'user'
          ? 'bg-blue-600 text-white'
          : 'bg-white dark:bg-gray-800 text-gray-800 dark:text-white border border-gray-200 dark:border-gray-700'}">
          <pre class="whitespace-pre-wrap font-sans text-sm">{message.content}</pre>
        </div>
      </div>
    {/each}

    {#if isLoading}
      <div class="flex justify-start">
        <div class="bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 p-3 rounded-lg">
          <div class="flex items-center gap-2 text-gray-500">
            <div class="animate-pulse">●</div>
            <span class="text-sm">AI 正在思考...</span>
          </div>
        </div>
      </div>
    {/if}
  </div>

  <!-- Generated Prompt Preview -->
  {#if isComplete && generatedPrompt}
    <div class="p-4 bg-green-50 dark:bg-green-900/20 border-t border-green-200 dark:border-green-800">
      <div class="flex items-center gap-2 mb-2">
        <span class="text-green-600 dark:text-green-400">✓</span>
        <span class="font-medium text-green-800 dark:text-green-300">需求收集完成</span>
      </div>
      <details class="text-sm">
        <summary class="cursor-pointer text-green-700 dark:text-green-400 hover:underline">
          查看生成的 Prompt
        </summary>
        <pre class="mt-2 p-3 bg-white dark:bg-gray-800 rounded border text-gray-700 dark:text-gray-300 overflow-x-auto whitespace-pre-wrap text-xs max-h-48 overflow-y-auto">{generatedPrompt}</pre>
      </details>
    </div>
  {/if}

  <!-- Input Area -->
  <div class="p-4 bg-white dark:bg-gray-800 border-t border-gray-200 dark:border-gray-700">
    {#if !isComplete}
      <div class="flex gap-2">
        <textarea
          class="flex-1 p-3 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-900 text-gray-800 dark:text-white resize-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          rows="2"
          placeholder="输入你的回答..."
          bind:value={userInput}
          onkeydown={handleKeydown}
          disabled={isLoading}
        ></textarea>
        <button
          class="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg disabled:opacity-50 self-end"
          onclick={sendMessage}
          disabled={isLoading || !userInput.trim()}
        >
          发送
        </button>
      </div>
    {:else}
      <!-- Completion Options -->
      <div class="space-y-3">
        <div class="flex gap-4">
          <div class="flex-1">
            <label class="block text-sm text-gray-600 dark:text-gray-400 mb-1">CLI</label>
            <select
              class="w-full p-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-900 text-gray-800 dark:text-white"
              bind:value={selectedCli}
            >
              <option value="claude">Claude Code</option>
              <option value="codex">Codex</option>
            </select>
          </div>
          <div class="flex-1">
            <label class="block text-sm text-gray-600 dark:text-gray-400 mb-1">最大迭代次数</label>
            <input
              type="number"
              class="w-full p-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-900 text-gray-800 dark:text-white"
              bind:value={maxIterations}
              min="1"
              max="100"
            />
          </div>
        </div>
        <div class="flex justify-end gap-2">
          <button
            class="px-4 py-2 text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-white"
            onclick={onCancel}
          >
            取消
          </button>
          <button
            class="px-6 py-2 bg-green-600 hover:bg-green-700 text-white rounded-lg disabled:opacity-50"
            onclick={handleComplete}
            disabled={isLoading}
          >
            开始执行 →
          </button>
        </div>
      </div>
    {/if}
  </div>
</div>
