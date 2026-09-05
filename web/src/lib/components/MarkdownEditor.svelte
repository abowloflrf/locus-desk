<script lang="ts">
  import type {
    Compartment as CodeMirrorCompartment,
    Extension as CodeMirrorExtension,
  } from '@codemirror/state';
  import type { EditorView as CodeMirrorEditorView } from 'codemirror';
  import { onMount } from 'svelte';

  let {
    value = $bindable(),
    disabled = false,
    id,
    label = 'Markdown editor',
    invalid = false,
    describedBy,
    onCancel,
    onSave,
  }: {
    value: string;
    disabled?: boolean;
    id: string;
    label?: string;
    invalid?: boolean;
    describedBy?: string;
    onCancel: () => void;
    onSave: () => void;
  } = $props();

  let host = $state<HTMLDivElement | null>(null);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let view = $state.raw<CodeMirrorEditorView | null>(null);
  let editableCompartment = $state.raw<CodeMirrorCompartment | null>(null);
  let editableExtension: ((editable: boolean) => CodeMirrorExtension) | null = null;
  let currentDisabled = false;

  $effect(() => {
    if (!view) return;
    view.contentDOM.setAttribute('aria-invalid', String(invalid));
    if (describedBy) view.contentDOM.setAttribute('aria-describedby', describedBy);
    else view.contentDOM.removeAttribute('aria-describedby');
  });

  onMount(() => {
    let destroyed = false;

    async function setup(): Promise<void> {
      try {
        await createEditor();
      } catch {
        if (destroyed) return;
        loading = false;
        loadError = 'Unable to load the Markdown editor.';
      }
    }

    async function createEditor(): Promise<void> {
      const [core, markdownModule, state, language, highlight, editorView] = await Promise.all([
        import('codemirror'),
        import('@codemirror/lang-markdown'),
        import('@codemirror/state'),
        import('@codemirror/language'),
        import('@lezer/highlight'),
        import('@codemirror/view'),
      ]);
      if (destroyed || !host) return;

      const { EditorView, basicSetup } = core;
      const compartment = new state.Compartment();
      const codeLine = editorView.Decoration.line({ class: 'cm-memo-code-line' });
      const codeLines = editorView.ViewPlugin.fromClass(
        class {
          decorations: import('@codemirror/view').DecorationSet;

          constructor(view: CodeMirrorEditorView) {
            this.decorations = this.collect(view);
          }

          collect(view: CodeMirrorEditorView) {
            const lines = new Set<number>();
            language.syntaxTree(view.state).iterate({
              enter(node) {
                if (node.name !== 'FencedCode' && node.name !== 'CodeBlock') return;
                const first = view.state.doc.lineAt(node.from).number;
                const last = view.state.doc.lineAt(node.to).number;
                for (let number = first; number <= last; number++) {
                  lines.add(view.state.doc.line(number).from);
                }
                return false;
              },
            });
            return editorView.Decoration.set(
              [...lines].sort((a, b) => a - b).map((from) => codeLine.range(from)),
            );
          }

          update(update: import('@codemirror/view').ViewUpdate) {
            if (
              update.docChanged ||
              language.syntaxTree(update.startState) !== language.syntaxTree(update.state)
            ) {
              this.decorations = this.collect(update.view);
            }
          }
        },
        { decorations: (plugin) => plugin.decorations },
      );
      const markdownHighlighting = language.HighlightStyle.define([
        { tag: highlight.tags.heading, color: 'var(--foreground)', fontWeight: '680' },
        { tag: highlight.tags.strong, color: 'var(--foreground)', fontWeight: '700' },
        { tag: highlight.tags.emphasis, fontStyle: 'italic' },
        {
          tag: [highlight.tags.link, highlight.tags.url],
          color: 'var(--primary)',
          textDecoration: 'underline',
        },
        {
          tag: [highlight.tags.meta, highlight.tags.punctuation],
          color: 'var(--muted-foreground)',
        },
        { tag: highlight.tags.quote, color: 'var(--muted-foreground)' },
        {
          tag: highlight.tags.monospace,
          color: 'var(--foreground)',
          fontFamily: 'var(--font-mono)',
          class: 'cm-memo-code',
        },
      ]);
      const editorTheme = EditorView.theme({
        '&': {
          color: 'var(--foreground)',
          backgroundColor: 'transparent',
          fontSize: '15px',
        },
        '&.cm-focused': { outline: 'none' },
        '.cm-scroller': {
          minHeight: '120px',
          maxHeight: 'min(420px, 56vh)',
          overflow: 'auto',
          fontFamily: 'var(--font-sans)',
          lineHeight: '24px',
          scrollbarColor: 'var(--border) transparent',
        },
        '.cm-content': {
          minHeight: '120px',
          padding: '8px 0 12px',
          caretColor: 'var(--foreground)',
        },
        '.cm-line': { padding: '0 6px' },
        '.cm-memo-code': { fontSize: '0.86em' },
        '.cm-memo-code-line': {
          fontFamily: 'var(--font-mono)',
          fontSize: '0.86em',
          lineHeight: '1.55',
        },
        '.cm-memo-code-line .cm-memo-code': { fontSize: 'inherit' },
        '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--primary)' },
        '.cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection': {
          backgroundColor: 'color-mix(in oklch, var(--primary), transparent 82%)',
        },
        '.cm-gutters': {
          display: 'none',
        },
        '.cm-activeLine': {
          backgroundColor: 'color-mix(in oklch, var(--muted), transparent 48%)',
        },
        '.cm-foldPlaceholder': {
          color: 'var(--muted-foreground)',
          backgroundColor: 'var(--muted)',
          border: 'none',
        },
        '.cm-panels, .cm-tooltip': {
          color: 'var(--foreground)',
          backgroundColor: 'var(--popover)',
          borderColor: 'var(--border)',
        },
        '.cm-panels.cm-panels-top': { borderBottomColor: 'var(--border)' },
        '.cm-searchMatch': {
          backgroundColor: 'color-mix(in oklch, var(--primary), transparent 76%)',
        },
        '.cm-searchMatch.cm-searchMatch-selected': {
          backgroundColor: 'color-mix(in oklch, var(--primary), transparent 62%)',
        },
      });
      const keyboardHandlers = EditorView.domEventHandlers({
        keydown(event) {
          if (event.key === 'Escape') {
            event.preventDefault();
            onCancel();
            return true;
          }
          if (!disabled && (event.metaKey || event.ctrlKey) && event.key === 'Enter') {
            event.preventDefault();
            onSave();
            return true;
          }
          return false;
        },
      });

      editableCompartment = compartment;
      editableExtension = (editable) => EditorView.editable.of(editable);
      view = new EditorView({
        doc: value,
        parent: host,
        extensions: [
          keyboardHandlers,
          basicSetup,
          markdownModule.markdown(),
          codeLines,
          language.syntaxHighlighting(markdownHighlighting),
          EditorView.lineWrapping,
          EditorView.contentAttributes.of({
            'aria-label': label,
            autocapitalize: 'sentences',
            id,
            spellcheck: 'true',
          }),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) value = update.state.doc.toString();
          }),
          compartment.of(EditorView.editable.of(!disabled)),
          editorTheme,
        ],
      });
      loading = false;
      view.dispatch({ selection: { anchor: view.state.doc.length } });
      view.focus();
    }

    void setup();
    return () => {
      destroyed = true;
      view?.destroy();
      view = null;
      editableCompartment = null;
      editableExtension = null;
    };
  });

  $effect(() => {
    const nextDisabled = disabled;
    if (view && editableCompartment && editableExtension && nextDisabled !== currentDisabled) {
      view.dispatch({
        effects: editableCompartment.reconfigure(editableExtension(!nextDisabled)),
      });
    }
    currentDisabled = nextDisabled;
  });

  $effect(() => {
    const nextValue = value;
    if (!view || view.state.doc.toString() === nextValue) return;
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: nextValue } });
  });
</script>

<div class="markdown-editor">
  <div
    aria-busy={loading}
    bind:this={host}
    class:failed={Boolean(loadError)}
    class="editor-mount"
  ></div>
  {#if loadError}<p class="editor-load-error" role="alert">{loadError}</p>{/if}
</div>

<style>
  .markdown-editor {
    min-width: 0;
    min-height: 120px;
    overflow: hidden;
    border-radius: var(--radius-md);
  }

  .editor-mount {
    min-height: 120px;
  }

  .editor-mount.failed {
    display: none;
  }

  .editor-load-error {
    display: flex;
    min-height: 120px;
    align-items: center;
    justify-content: center;
    margin: 0;
    color: var(--destructive);
    font-size: 13px;
  }

  @media (max-width: 767px) {
    .markdown-editor :global(.cm-editor) {
      font-size: 16px;
    }
  }
</style>
