mod error;
mod utils;
use error::Error;

use mdzk::{Vault, VaultBuilder};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{RwLock, RwLockWriteGuard};
use tower_lsp::jsonrpc::{ErrorCode, Result};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Default)]
struct State {
    current_ident: String,
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

    async fn update_vault(
        &self,
        state: &mut RwLockWriteGuard<'_, State>,
    ) -> std::result::Result<(), Error> {
        state.vault = Some(
            VaultBuilder::default()
                .source(state.root.as_ref().unwrap())
                .build()?,
        );

        Ok(())
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(root) = params.root_uri {
            if let Ok(root) = root.to_file_path() {
                let mut state = self.state.write().await;

                state.root = Some(root);
                self.update_vault(&mut state).await?;

                return Ok(InitializeResult {
                    server_info: Some(ServerInfo {
                        name: "mdzk-language-server".to_owned(),
                        version: Some(env!("CARGO_PKG_VERSION").to_owned()),
                    }),

                    capabilities: ServerCapabilities {
                        text_document_sync: Some(
                            // TODO: Investigate TextDocumentSyncKind::INCREMENTAL
                            TextDocumentSyncCapability::Options(TextDocumentSyncOptions{
                                change: Some(TextDocumentSyncKind::INCREMENTAL),
                                save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                                ..Default::default()
                            }),
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
                });
            }
        }

        Err(tower_lsp::jsonrpc::Error {
            code: ErrorCode::InternalError,
            message: "mdzk-language-server needs a workspace to load a vault".to_owned(),
            data: None,
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "mdzk-language-server initialized!")
            .await;
    }

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        let state = self.state.read().await;

        let complete_note = state.current_ident == "[[";

        if complete_note {
            return Ok(Some(CompletionResponse::Array(
                state
                    .vault
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|(_, note)| CompletionItem {
                        label: note.title.clone(),
                        detail: note.path.as_ref().map(|p| p.to_string_lossy().to_string()),
                        documentation: Some(Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: note.content.clone(),
                        })),
                        kind: Some(CompletionItemKind::FILE),
                        insert_text: Some(format!("{}]]", note.title)),
                        ..Default::default()
                    })
                    .collect(),
            )));
        }

        Ok(None)
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let mut state = self.state.write().await;
        let current_char = params.content_changes[0].text.as_str();
        match current_char {
            " " | "" | "\n" | "\t" | "\r" => state.current_ident = String::new(),
            c => state.current_ident.push_str(c),
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        // Update vault on file save
        let mut state = self.state.write().await;

        if let Err(err) = self.update_vault(&mut state).await {
            self.client
                .log_message(MessageType::ERROR, err.to_string())
                .await
        } else {
            let cur_uri = params.text_document.uri;
            // FIXME: Handle errors
            let cur_path = cur_uri.to_file_path().unwrap();
            self.client.show_message(MessageType::INFO, format!("{:?}", cur_path)).await;
            let cur_id = state.vault.as_ref().unwrap().id_of("Action").unwrap(); // temp
            let cur_note = state.vault.as_ref().unwrap().get(&cur_id).unwrap();
            self.client.publish_diagnostics(
                cur_uri,
                cur_note.invalid_internal_links
                    .iter()
                    .map(|(range, link_string)| {
                        Diagnostic {
                            severity: Some(DiagnosticSeverity::WARNING),
                            source: Some("mdzk".to_owned()),
                            message: format!("Missing link destination: {}", link_string),
                            range: utils::range_to_lsp_range(range, &cur_note.content),
                            ..Default::default()
                        }
                    })
                    .collect(),
                None
            ).await;
        };
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
