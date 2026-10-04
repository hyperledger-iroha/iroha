//! Blocking collection reads driven by the facade's reusable runtime.

use std::{fmt, sync::Arc};

use norito::json::Value;

use super::{AccountClient, Client, RuntimeOwner};
use crate::{
    Result,
    collections::{Collection, CollectionReader, ListQuery, Page, Pager},
};

impl Client {
    /// Read one page of a collection, signed by this facade's account when it
    /// can sign directly.
    ///
    /// This is [`crate::client::AccountClient::list_page`] on the facade's
    /// reusable runtime.
    ///
    /// ```no_run
    /// use iroha::collections::{Collection, ListQuery, field};
    ///
    /// # fn example(client: &iroha::blocking::Client) -> iroha::Result<()> {
    /// let query = ListQuery::new().filter(field("id").ne("genesis")).limit(100);
    /// let page = client.list_page(&Collection::Accounts, &query)?;
    /// for account in client.list(Collection::Accounts, query) {
    ///     println!("{:?}", account?.get("id"));
    /// }
    /// # let _ = page;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    /// Returns the errors of the asynchronous read, and [`crate::Error::Blocking`]
    /// when called inside an asynchronous runtime.
    pub fn list_page(&self, collection: &Collection, query: &ListQuery) -> Result<Page<Value>> {
        self.runtime
            .block_on(self.account.list_page(collection, query))?
    }

    /// Lazily read every row of a collection, following `next_cursor` page by page.
    ///
    /// Each page is fetched on the facade's runtime only when the iterator
    /// needs it; requests are signed as by [`Self::list_page`].
    pub fn list(&self, collection: Collection, query: ListQuery) -> CollectionIter {
        CollectionIter::new(
            Arc::clone(&self.runtime),
            CollectionReader::Account(self.account.clone()),
            collection,
            query,
        )
    }
}

impl AccountClient {
    /// Read one page of a collection with this account's canonical request signature.
    ///
    /// # Errors
    /// Returns the errors of [`crate::client::AccountClient::list_page`], and
    /// [`crate::Error::Blocking`] when called inside an asynchronous runtime.
    pub fn list_page(&self, collection: &Collection, query: &ListQuery) -> Result<Page<Value>> {
        self.runtime
            .block_on(self.inner.list_page(collection, query))?
    }

    /// Lazily read every row of a collection with signed page requests.
    pub fn list(&self, collection: Collection, query: ListQuery) -> CollectionIter {
        CollectionIter::new(
            Arc::clone(&self.runtime),
            CollectionReader::Account(self.inner.clone()),
            collection,
            query,
        )
    }
}

/// Every row of one collection query, fetched lazily page by page.
///
/// Created by [`Client::list`] or [`AccountClient::list`]. Rows arrive in the
/// requested order; the next page is requested only after the current one is
/// consumed. Short or empty pages with a `next_cursor` are followed; the first
/// error ends the iteration.
pub struct CollectionIter {
    runtime: Arc<RuntimeOwner>,
    pager: Pager,
}

impl CollectionIter {
    fn new(
        runtime: Arc<RuntimeOwner>,
        reader: CollectionReader,
        collection: Collection,
        query: ListQuery,
    ) -> Self {
        Self {
            runtime,
            pager: Pager::new(reader, collection, query),
        }
    }

    /// The collection this iterator reads.
    pub const fn collection(&self) -> &Collection {
        &self.pager.collection
    }

    /// Exact number of matching rows reported by the latest page that carried
    /// one; present only when the query asked for [`ListQuery::include_total`].
    pub const fn total(&self) -> Option<u64> {
        self.pager.total()
    }
}

impl fmt::Debug for CollectionIter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CollectionIter")
            .field("collection", &self.pager.collection)
            .field("buffered", &self.pager.buffered())
            .field("total", &self.pager.total())
            .finish_non_exhaustive()
    }
}

impl Iterator for CollectionIter {
    type Item = Result<Value>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(row) = self.pager.pop() {
                return Some(Ok(row));
            }
            let request = self.pager.take_request()?;
            let page = self
                .runtime
                .block_on(self.pager.reader.page(&self.pager.collection, &request))
                .map_err(crate::Error::from)
                .and_then(|page| page);
            if let Err(error) = page.and_then(|page| self.pager.accept(&request, page)) {
                self.pager.finish();
                return Some(Err(error));
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.pager.buffered(), None)
    }
}

impl std::iter::FusedIterator for CollectionIter {}
