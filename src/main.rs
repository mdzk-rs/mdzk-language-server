use mdzk::{Vault, VaultBuilder};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::{Result, Error, ErrorCode};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LspService, Server, LanguageServer};

#[derive(Default)]
struct State {
    root: Option<PathBuf>,
    vault: Option<Vault>,
}

struct Backend {
    client: Client,
    state: Arc<RwLock<State>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            state: Arc::new(RwLock::new(State::default())),
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(root) = params.root_uri {
            let mut state = self.state.write().await;

            state.root = Some(root.to_file_path().unwrap()); // FIXME: Handle error
            state.vault = Some(VaultBuilder::default()
                .source(state.root.as_ref().unwrap())
                .build()
                .unwrap()); // FIXME: Handle error

            Ok(InitializeResult {
                server_info: Some(ServerInfo {
                    name: "mdzk-language-server".to_owned(),
                    version: Some(env!("CARGO_PKG_VERSION").to_owned()),
                }),

                capabilities: ServerCapabilities {
                    text_document_sync: Some(
                        // TODO: Investigate TextDocumentSyncKind::INCREMENTAL
                        TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)
                    ),
                    hover_provider: Some(HoverProviderCapability::Simple(true)),
                    completion_provider: Some(CompletionOptions {
                        resolve_provider: Some(false),
                        trigger_characters: Some(vec!["[".to_owned(), "#".to_owned()]),
                        all_commit_characters: None,
                        work_done_progress_options: Default::default(),
                    }),
                    document_link_provider: Some(DocumentLinkOptions {
                        resolve_provider: Some(false),
                        work_done_progress_options: Default::default(),
                    }),
                    ..Default::default()
                },
            })
        } else {
            Err(Error {
                code: ErrorCode::InternalError,
                message: "mdzk-language-server needs a workspace to load a vault".to_owned(),
                data: None,
            })
        }
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "mdzk-language-server initialized!")
            .await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let state = self.state.read().await;

        if let Some(context) = params.context {
            let complete_note = match context {
                CompletionContext {
                    trigger_kind: CompletionTriggerKind::INVOKED,
                    trigger_character: _,
                } => false,

                CompletionContext {
                    trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                    trigger_character: Some(c),
                } if c == "[" => true,

                _ => false,
            };

            if complete_note {
                return Ok(Some(CompletionResponse::Array(
                    state.vault.as_ref().unwrap().iter().map(|(_, note)| {
                        CompletionItem {
                            label: note.title.clone(),
                            detail: note.path.as_ref().map(|p| p.to_string_lossy().to_string()),
                            documentation: Some(Documentation::MarkupContent(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: note.content.clone(),
                            })),
                            kind: Some(CompletionItemKind::FILE),
                            insert_text: Some(format!("{}]]", note.title)),
                            ..Default::default()
                        }
                    }).collect()
                )))
            }
        }

        Ok(None)
    }

    async fn did_create_files(&self, _: CreateFilesParams) {
        // Create a new state (build vault again) whenever files are created
        let mut state = self.state.write().await;
        state.vault = Some(VaultBuilder::default()
            .source(state.root.as_ref().unwrap())
            .build()
            .unwrap()); // FIXME: Handle error
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
