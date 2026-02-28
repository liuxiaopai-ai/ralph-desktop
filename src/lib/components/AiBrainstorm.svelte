<script lang="ts">
  import type { ProjectState, CliType } from '$lib/types';
  import * as api from '$lib/services/tauri';
  import type { ConversationMessage, AiBrainstormResponse, QuestionOption } from '$lib/services/tauri';
  import { config } from '$lib/stores/settings';
  import { updateProjectName } from '$lib/stores/projects';
  import { _ } from 'svelte-i18n';

  interface Props {
    project: ProjectState;
    onComplete: (project: ProjectState) => void;
    onCancel: () => void;
  }

  let { project, onComplete, onCancel }: Props = $props();

  let conversation = $state<ConversationMessage[]>([]);
  let currentQuestion = $state<AiBrainstormResponse | null>(null);

  // Store question + user's selection together
  interface HistoryEntry {
    question: AiBrainstormResponse;
    selectedOptions: Set<string>;
    customInput: string;
    showOtherInput: boolean;
    userAnswer: string;
  }
  let history = $state<HistoryEntry[]>([]);

  let selectedOptions = $state<Set<string>>(new Set());
  let customInput = $state('');
  let showOtherInput = $state(false);
  let isLoading = $state(false);
  let lastError = $state<string | null>(null);
  let generatedPrompt = $state<string | null>(null);
  let selectedCli = $state<CliType>($config.defaultCli);
  let maxIterations = $state($config.defaultMaxIterations);
  let titleGenerated = $state(false);


  // Start with initial question
  $effect(() => {
    if (conversation.length === 0 && !currentQuestion) {
      askInitialQuestion();
    }
  });

  async function askInitialQuestion() {
    isLoading = true;
    try {
      // First question: what do you want to do?
      currentQuestion = {
        question: $_('brainstorm.initialQuestion'),
        description: $_('brainstorm.initialDescription'),
        options: [],
        multiSelect: false,
        allowOther: false,
        isComplete: false
      };
    } finally {
      isLoading = false;
    }
  }

  async function submitAnswer(answer: string) {
    if (isLoading) return;

    lastError = null;

    // Save current question and user's selection to history
    if (currentQuestion) {
      history = [...history, {
        question: currentQuestion,
        selectedOptions: new Set(selectedOptions),
        customInput: customInput,
        showOtherInput: showOtherInput,
        userAnswer: answer
      }];
    }

    // Add to conversation
    conversation = [...conversation, { role: 'user', content: answer }];

    // Fire-and-forget title generation on the first user message only
    if (!titleGenerated) {
      titleGenerated = true;
      api.generateProjectTitle(project.id, answer).then((title) => {
        updateProjectName(project.id, title);
      }).catch(() => {
        // Fallback: truncate first message to 15 chars + ellipsis
        const fallback = answer.length > 15 ? answer.slice(0, 15) + '…' : answer;
        updateProjectName(project.id, fallback);
      });
    }

    // Reset state
    selectedOptions = new Set();
    customInput = '';
    showOtherInput = false;
    isLoading = true;

    try {
      const response = await api.aiBrainstormChat(project.id, conversation);

      if (response.isComplete && response.generatedPrompt) {
        generatedPrompt = response.generatedPrompt;
        currentQuestion = null;
        lastError = null;
      } else {
        // Add AI response to conversation for context
        conversation = [...conversation, { role: 'assistant', content: response.question }];
        currentQuestion = response;
        lastError = null;
      }
    } catch (error) {
      console.error('Failed to get AI response:', error);
      lastError = error instanceof Error ? error.message : String(error);
      // Fallback question
      currentQuestion = {
        question: $_('brainstorm.fallbackQuestion'),
        description: $_('brainstorm.fallbackDescription'),
        options: [],
        multiSelect: false,
        allowOther: false,
        isComplete: false
      };
    } finally {
      isLoading = false;
    }
  }

  function handleOptionClick(option: QuestionOption) {
    if (currentQuestion?.multiSelect) {
      const newSet = new Set(selectedOptions);
      if (newSet.has(option.value)) {
        newSet.delete(option.value);
      } else {
        newSet.add(option.value);
      }
      selectedOptions = newSet;
    } else {
      // Single select - submit immediately
      submitAnswer(option.label);
    }
  }

  function handleOtherClick() {
    if (currentQuestion?.multiSelect) {
      showOtherInput = !showOtherInput;
    } else {
      showOtherInput = true;
    }
  }

  function handleSubmitMultiple() {
    const answers = Array.from(selectedOptions);
    if (showOtherInput && customInput.trim()) {
      answers.push(customInput.trim());
    }
    if (answers.length > 0) {
      submitAnswer(answers.join(', '));
    }
  }

  function handleSubmitText() {
    if (customInput.trim()) {
      submitAnswer(customInput.trim());
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      if (event.isComposing) {
        return;
      }
      event.preventDefault();
      if (currentQuestion?.options.length === 0 || showOtherInput) {
        handleSubmitText();
      }
    }
    if (event.key === 'Escape') {
      onCancel();
    }
  }

  // Check if we can go back
  const canGoBack = $derived(history.length > 0 || generatedPrompt !== null);

  function handleGoBack() {
    if (isLoading) return;

    // If we're at the completion state, go back to last question
    if (generatedPrompt) {
      generatedPrompt = null;
      // Restore last entry from history
      if (history.length > 0) {
        const lastEntry = history[history.length - 1];
        currentQuestion = lastEntry.question;
        selectedOptions = new Set(lastEntry.selectedOptions);
        customInput = lastEntry.customInput;
        showOtherInput = lastEntry.showOtherInput;
        history = history.slice(0, -1);
        // Remove last user answer from conversation (no assistant response for completion)
        if (conversation.length >= 1) {
          conversation = conversation.slice(0, -1);
        }
      } else {
        // No history, go back to initial question
        askInitialQuestion();
      }
      return;
    }

    // Normal case: go back one question
    if (history.length > 0) {
      // Restore previous question and user's selection
      const lastEntry = history[history.length - 1];
      currentQuestion = lastEntry.question;
      selectedOptions = new Set(lastEntry.selectedOptions);
      customInput = lastEntry.customInput;
      showOtherInput = lastEntry.showOtherInput;
      history = history.slice(0, -1);

      // Remove last user answer and the assistant question before it
      if (conversation.length >= 2) {
        conversation = conversation.slice(0, -2);
      }
    } else {
      // No history, already at initial question - do nothing or reset
      askInitialQuestion();
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
</script>

<div class="flex-1 flex flex-col h-full bg-vscode-editor">
  <!-- Header -->
  <div class="px-6 pt-4 pb-3 border-b border-vscode">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-2">
        <span class="text-sm text-vscode">{$_('brainstorm.title')}</span>
        {#if conversation.length > 0}
          <span class="text-xs text-vscode-muted">
            · {$_('brainstorm.questionLabel')} {Math.floor(conversation.filter(m => m.role === 'user').length) + (generatedPrompt ? 0 : 1)}
          </span>
        {/if}
      </div>
      {#if generatedPrompt}
        <span class="text-xs text-[#4ec9b0]">{$_('brainstorm.completeLabel')}</span>
      {:else if isLoading}
        <span class="text-xs text-vscode-muted">{$_('brainstorm.thinkingLabel')}</span>
      {/if}
    </div>
  </div>

  <!-- Main content -->
  <div class="flex-1 overflow-y-auto px-6 py-6">
    {#if isLoading}
      <div class="flex items-center gap-3 text-vscode-dim">
        <div class="animate-spin h-4 w-4 border-2 border-vscode-border border-t-vscode-accent rounded-full"></div>
        <span class="text-sm">{$_('brainstorm.thinkingLabel')}</span>
      </div>
    {:else if generatedPrompt}
      <!-- Completion state -->
      <div class="space-y-4">
        <div>
          <h2 class="text-base font-medium text-vscode mb-1">{$_('brainstorm.requirementsComplete')}</h2>
          <p class="text-sm text-vscode-dim">{$_('brainstorm.generatedPromptDesc')}</p>
        </div>

        <div class="bg-vscode-input rounded p-3 max-h-48 overflow-y-auto border border-vscode">
          <pre class="text-xs text-vscode whitespace-pre-wrap font-mono">{generatedPrompt}</pre>
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div>
            <label class="block text-xs text-vscode-dim mb-1.5">{$_('brainstorm.cliLabel')}</label>
            <select
              class="w-full h-10 px-3 text-sm rounded border border-vscode bg-vscode-input"
              bind:value={selectedCli}
            >
              <option value="claude">Claude Code</option>
              <option value="codex">Codex</option>
              <option value="opencode">OpenCode</option>
            </select>
          </div>
          <div>
            <label class="block text-xs text-vscode-dim mb-1.5">{$_('brainstorm.maxIterationsLabel')}</label>
            <input
              type="number"
              class="w-full h-10 px-3 text-sm rounded border border-vscode bg-vscode-input"
              bind:value={maxIterations}
              min="1"
              max="100"
            />
          </div>
        </div>
      </div>
    {:else if currentQuestion}
      <!-- Question display -->
      <div class="space-y-4">
        <div>
          <h2 class="text-base font-medium text-vscode mb-1">{currentQuestion.question}</h2>
          {#if currentQuestion.description}
            <p class="text-sm text-vscode-dim">{currentQuestion.description}</p>
          {/if}
          {#if lastError}
            <div class="mt-2 rounded border border-[#f14c4c40] bg-[#f14c4c14] p-3">
              <pre class="text-xs text-[#f14c4c] whitespace-pre-wrap">{lastError}</pre>
            </div>
          {/if}
        </div>

        <!-- Options as cards -->
        {#if currentQuestion.options.length > 0}
          <div class="space-y-2">
            {#each currentQuestion.options as option}
              <button
                class="w-full text-left px-3 py-2.5 rounded border transition-all text-sm
                  {selectedOptions.has(option.value)
                    ? 'bg-vscode-selection border-vscode-accent text-vscode'
                    : 'bg-vscode-input border-vscode text-vscode hover:border-vscode-light hover:bg-vscode-hover'}"
                onclick={() => handleOptionClick(option)}
              >
                <div class="flex items-start gap-3">
                  <!-- Radio (circle) for single select, Checkbox (square) for multi select -->
                  <div class="mt-0.5 w-4 h-4 border flex items-center justify-center flex-shrink-0
                    {currentQuestion.multiSelect ? 'rounded' : 'rounded-full'}
                    {selectedOptions.has(option.value) ? 'border-vscode-accent bg-vscode-accent' : 'border-vscode-border'}">
                    {#if selectedOptions.has(option.value)}
                      {#if currentQuestion.multiSelect}
                        <svg class="w-2.5 h-2.5 text-white" fill="currentColor" viewBox="0 0 20 20">
                          <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
                        </svg>
                      {:else}
                        <div class="w-2 h-2 bg-white rounded-full"></div>
                      {/if}
                    {/if}
                  </div>
                  <div>
                    <div class="font-medium">{option.label}</div>
                    {#if option.description}
                      <div class="text-xs text-vscode-muted mt-0.5">{option.description}</div>
                    {/if}
                  </div>
                </div>
              </button>
            {/each}

            <!-- Other option -->
            {#if currentQuestion.allowOther}
              <button
                class="w-full text-left px-3 py-2.5 rounded border transition-all text-sm
                  {showOtherInput
                    ? 'bg-vscode-selection border-vscode-accent text-vscode'
                    : 'bg-vscode-input border-vscode text-vscode hover:border-vscode-light'}"
                onclick={handleOtherClick}
              >
                <div class="flex items-start gap-3">
                  <div class="mt-0.5 w-4 h-4 rounded border flex items-center justify-center flex-shrink-0
                    {showOtherInput ? 'border-vscode-accent bg-vscode-accent' : 'border-vscode-border'}">
                    {#if showOtherInput}
                      <svg class="w-2.5 h-2.5 text-white" fill="currentColor" viewBox="0 0 20 20">
                        <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
                      </svg>
                    {/if}
                  </div>
                  <div class="font-medium">{$_('brainstorm.otherLabel')}</div>
                </div>
              </button>
            {/if}
          </div>

          <!-- Custom input for "Other" -->
          {#if showOtherInput}
            <div class="mt-3">
              <textarea
                class="w-full p-3 rounded text-sm resize-none"
                rows="2"
                placeholder={$_('brainstorm.answerPlaceholder')}
                bind:value={customInput}
                onkeydown={handleKeydown}
                data-testid="brainstorm-input"
              ></textarea>
            </div>
          {/if}
        {:else}
          <!-- Text input only -->
          <div>
            <textarea
              class="w-full p-3 rounded text-sm resize-none"
              rows="3"
              placeholder={$_('brainstorm.answerPlaceholder')}
              bind:value={customInput}
              onkeydown={handleKeydown}
              data-testid="brainstorm-input"
            ></textarea>
          </div>
        {/if}
      </div>
    {:else}
      <!-- Fallback: no question loaded -->
      <div class="flex items-center gap-3 text-vscode-dim">
        <div class="animate-spin h-4 w-4 border-2 border-vscode-border border-t-vscode-accent rounded-full"></div>
        <span class="text-sm">{$_('brainstorm.loadingLabel')}</span>
      </div>
    {/if}
  </div>

  <!-- Footer -->
  <div class="px-6 py-3 border-t border-vscode flex items-center justify-between">
    <div class="flex items-center gap-3">
      {#if canGoBack && !isLoading}
        <button
          class="flex items-center gap-1.5 px-3 py-1.5 text-sm text-vscode-dim hover:text-vscode hover:bg-vscode-hover rounded transition-colors"
          onclick={handleGoBack}
        >
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/>
          </svg>
          {$_('brainstorm.backLabel')}
        </button>
      {/if}
      <span class="text-xs text-vscode-muted">
        {#if !generatedPrompt}
          <kbd class="px-1.5 py-0.5 bg-vscode-input rounded text-xs border border-vscode">Esc</kbd>{$_('brainstorm.escToCancelSuffix')}
        {/if}
      </span>
    </div>

    <div class="flex gap-2">
      {#if generatedPrompt}
        <button
          class="px-3 py-1.5 text-sm text-vscode-dim hover:text-vscode"
          onclick={onCancel}
        >
          {$_('brainstorm.cancelLabel')}
        </button>
        <button
          class="px-4 py-1.5 bg-vscode-accent hover:bg-vscode-accent-hover text-white rounded text-sm disabled:opacity-50"
          onclick={handleComplete}
          disabled={isLoading}
          data-testid="brainstorm-start"
        >
          {$_('brainstorm.startExecution')}
        </button>
      {:else if currentQuestion}
        {#if currentQuestion.multiSelect && (selectedOptions.size > 0 || (showOtherInput && customInput.trim()))}
          <button
            class="px-4 py-1.5 bg-vscode-accent hover:bg-vscode-accent-hover text-white rounded text-sm"
            onclick={handleSubmitMultiple}
          data-testid="brainstorm-submit"
          >
            <span class="opacity-70 mr-1">{selectedOptions.size + (showOtherInput && customInput.trim() ? 1 : 0)}</span>
            {$_('brainstorm.submitLabel')}
          </button>
        {:else if currentQuestion.options.length === 0}
          <button
            class="px-4 py-1.5 bg-vscode-accent hover:bg-vscode-accent-hover text-white rounded text-sm disabled:opacity-50"
            onclick={handleSubmitText}
            disabled={!customInput.trim()}
            data-testid="brainstorm-submit"
          >
            {$_('brainstorm.continueLabel')}
          </button>
        {:else if showOtherInput && !currentQuestion.multiSelect}
          <button
            class="px-4 py-1.5 bg-vscode-accent hover:bg-vscode-accent-hover text-white rounded text-sm disabled:opacity-50"
            onclick={handleSubmitText}
            disabled={!customInput.trim()}
            data-testid="brainstorm-submit"
          >
            {$_('brainstorm.continueLabel')}
          </button>
        {/if}
      {/if}
    </div>
  </div>
</div>
