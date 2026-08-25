import React from "react";
import { useTranslation } from "react-i18next";
import type { Editor } from "@tiptap/react";
import {
  Bold,
  Code2,
  Heading1,
  Heading2,
  Heading3,
  Italic,
  List,
  ListOrdered,
  Pilcrow,
  Quote,
  Redo2,
  Strikethrough,
  Underline as UnderlineIcon,
  Undo2,
} from "lucide-react";

const ToolbarButton: React.FC<{
  onClick: () => void;
  title: string;
  active?: boolean;
  disabled?: boolean;
  children: React.ReactNode;
}> = ({ onClick, title, active, disabled, children }) => (
  <button
    type="button"
    onClick={onClick}
    title={title}
    disabled={disabled}
    className={`p-1.5 rounded-md transition-colors cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed ${
      active
        ? "bg-logo-primary/20 text-logo-primary"
        : "text-text/60 hover:bg-mid-gray/10 hover:text-text"
    }`}
  >
    {children}
  </button>
);

/**
 * Rich-text formatting toolbar shared by the prompt editor surfaces (Dictate
 * tab, Prompt Workbench). Re-renders when the parent re-renders — parents bump
 * a counter on every editor transaction so active states stay fresh.
 */
export const PromptFormatToolbar: React.FC<{ editor: Editor | null }> = ({
  editor,
}) => {
  const { t } = useTranslation();
  if (!editor) return null;
  const tb = (titleKey: string) => t(`home.toolbar.${titleKey}`);

  return (
    <div className="flex items-center gap-0.5 flex-wrap px-2 py-1 border-b border-mid-gray/20">
      <ToolbarButton
        title={tb("undo")}
        disabled={!editor.can().undo()}
        onClick={() => editor.chain().focus().undo().run()}
      >
        <Undo2 className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("redo")}
        disabled={!editor.can().redo()}
        onClick={() => editor.chain().focus().redo().run()}
      >
        <Redo2 className="w-3.5 h-3.5" />
      </ToolbarButton>
      <div className="w-px h-4 mx-1 bg-mid-gray/30" />
      <ToolbarButton
        title={tb("paragraph")}
        active={editor.isActive("paragraph")}
        onClick={() => editor.chain().focus().setParagraph().run()}
      >
        <Pilcrow className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("heading1")}
        active={editor.isActive("heading", { level: 1 })}
        onClick={() => editor.chain().focus().toggleHeading({ level: 1 }).run()}
      >
        <Heading1 className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("heading2")}
        active={editor.isActive("heading", { level: 2 })}
        onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
      >
        <Heading2 className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("heading3")}
        active={editor.isActive("heading", { level: 3 })}
        onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()}
      >
        <Heading3 className="w-3.5 h-3.5" />
      </ToolbarButton>
      <div className="w-px h-4 mx-1 bg-mid-gray/30" />
      <ToolbarButton
        title={tb("bold")}
        active={editor.isActive("bold")}
        onClick={() => editor.chain().focus().toggleBold().run()}
      >
        <Bold className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("italic")}
        active={editor.isActive("italic")}
        onClick={() => editor.chain().focus().toggleItalic().run()}
      >
        <Italic className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("underline")}
        active={editor.isActive("underline")}
        onClick={() => editor.chain().focus().toggleUnderline().run()}
      >
        <UnderlineIcon className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("strike")}
        active={editor.isActive("strike")}
        onClick={() => editor.chain().focus().toggleStrike().run()}
      >
        <Strikethrough className="w-3.5 h-3.5" />
      </ToolbarButton>
      <div className="w-px h-4 mx-1 bg-mid-gray/30" />
      <ToolbarButton
        title={tb("bulletList")}
        active={editor.isActive("bulletList")}
        onClick={() => editor.chain().focus().toggleBulletList().run()}
      >
        <List className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("orderedList")}
        active={editor.isActive("orderedList")}
        onClick={() => editor.chain().focus().toggleOrderedList().run()}
      >
        <ListOrdered className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("blockquote")}
        active={editor.isActive("blockquote")}
        onClick={() => editor.chain().focus().toggleBlockquote().run()}
      >
        <Quote className="w-3.5 h-3.5" />
      </ToolbarButton>
      <ToolbarButton
        title={tb("codeBlock")}
        active={editor.isActive("codeBlock")}
        onClick={() => editor.chain().focus().toggleCodeBlock().run()}
      >
        <Code2 className="w-3.5 h-3.5" />
      </ToolbarButton>
    </div>
  );
};
