use std::borrow::BorrowMut;
use std::io::{ Read, Write };
use std::net::{ TcpListener, TcpStream };
use iced::{Rectangle, Scrollable, Subscription, button};
use iced::text_input;
use iced::{
    Button, Column, Text, Settings, Element, Align, TextInput, Row,
    Command, Application, Clipboard, executor,
};
use iced::time;
use std::sync::{ Mutex, Arc };

pub mod style {
    use iced::{button, Background, Color, Vector};

    pub enum Button {
        Primary,
        Secondary,
    }

    impl button::StyleSheet for Button {
        fn active(&self) -> button::Style {
            button::Style {
                background: Some(Background::Color(match self {
                    Button::Primary => Color::from_rgb(0.11, 0.42, 0.87),
                    Button::Secondary => Color::from_rgb(0.5, 0.5, 0.5),
                })),
                shadow_offset: Vector::new(1.0, 1.0),
                text_color: Color::from_rgb8(0xEE, 0xEE, 0xEE),
                ..button::Style::default()
            }
        }

        fn hovered(&self) -> button::Style {
            button::Style {
                text_color: Color::WHITE,
                background: Some(Background::Color(match self {
                    Button::Primary => Color::from_rgb(0.0, 0.222, 0.244),
                    Button::Secondary => Color::from_rgb(0.5, 0.5, 0.5),
                })),
                shadow_offset: Vector::new(1.0, 2.0),
                ..self.active()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    IncrementPressed,
    ListenInputChange(String),
    ProxyInputChange(String),
    Tick(i64),
    ShowRequest(Request),
}

#[derive(Default, Debug, Clone)]
pub struct Request {
    button: button::State,
    method: String,
    uri: String,
    request: String,
    response: String,
    time: std::time::Duration,
    selected: bool,
}

#[derive(Default, Clone)]
struct Window {
    value: bool,
    listen: String,
    listen_input: text_input::State,
    proxy_url: Arc<String>,
    proxy_input: text_input::State,
    left_scrollable: iced::scrollable::State,
    left_scrollable_request: Arc<Mutex<Vec<Request>>>,
    listen_button: button::State,
    request: String,
    response: String,

    requests: Vec<Request>,
}

impl Application for Window {
    type Message = Message;
    type Executor = executor::Default;
    type Flags = ();
    fn new(_flags: ()) -> (Self, Command<Message>) {
        let mut s = Self::default();
        s.value = true;
        (s, Command::none())
    }

    fn title(&self) -> String {
        String::from("Counter - Iced")
    }
    fn view(&mut self) -> Element<Message> {
        let mut column = Column::new()
            .padding(20)
            .align_items(Align::Center)
            .push(
                Row::new().push(
                    Text::new("Listen ").size(20)
                ).push(
                    TextInput::new(&mut self.listen_input, "0.0.0.0:8081", &self.listen, Message::ListenInputChange)
                ).push(
                    Text::new("Proxy ").size(20)
                ).push(
                    TextInput::new(&mut self.proxy_input, "127.0.0.1:8080", &self.proxy_url.as_str(), Message::ProxyInputChange),
                )
            );
        if self.value {
            column = column.push(
                Button::new(&mut self.listen_button, Text::new("Listen"))
                    .on_press(Message::IncrementPressed),
            )
        }
        let mut events = Column::new();
        self.requests.clear();
        if let Ok(requests) = self.left_scrollable_request.lock() {
            for item in requests.to_vec() {
                self.requests.push(item);
            }
        }
        for item in &mut self.requests {
            let req = item.clone();
            events = events.push(
                Button::new(
                    &mut item.button,
                    Text::new(format!("{} {} {:?}", item.method, item.uri, item.time)).size(15)
                ).style(style::Button::Primary).on_press(Message::ShowRequest(req))
            );
        }
        column = column.push(
            Row::new().push(
                Scrollable::new(&mut self.left_scrollable).push(
                    events
                )
            ).push(
                Text::new(self.request.as_str()).size(15).width(iced::Length::Fill)
            ).push(
                Text::new(self.response.as_str()).size(15).width(iced::Length::Fill)
            )
        );
        column.into()
    }

    fn update(&mut self, message: Message, _clipboard: &mut Clipboard) -> Command<Message> {
        match message {
            Message::IncrementPressed => {
                self.request = String::new();
                if self.value {
                    let mut proxy_addr = Arc::clone(&self.proxy_url);
                    let mut addr = self.listen.as_str();
                    if addr == "" {
                        addr = "0.0.0.0:8081";
                    }
                    if proxy_addr.as_str() == "" {
                        proxy_addr = Arc::new(String::from("127.0.0.1:8080"));
                    }
                    let request = Arc::clone(&self.left_scrollable_request);
                    let proxy_addr = Arc::clone(&proxy_addr);
                    self.value = !listen(addr, proxy_addr, request);
                }
            },
            Message::ListenInputChange(s) => {
                self.listen = s;
            },
            Message::ProxyInputChange(s) => {
                self.proxy_url = Arc::new(s);
            },
            Message::Tick(_) => {
            },
            Message::ShowRequest(req) => {
                self.request = req.request;
                self.response = req.response;
            }
        }
        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        time::every(std::time::Duration::from_millis(1000))
            .map(|_| Message::Tick(1))
    }
}

fn handle_connection(mut stream: &TcpStream, proxy_addr: &Arc<String>, mut request: Request) -> Result<Request, std::io::Error> {
    let mut buffer = [0; 1024];
    let mut target_stream = match TcpStream::connect(proxy_addr.to_string()) {
            Ok(stream) => {
            stream
        },
        Err(err) => {
            request.method = format!("{}", err);
            return Ok(request);
        }
    };

    request.request.clear();
    request.response.clear();
    request.method.push_str("Non-http protocol");
    let start_time = std::time::SystemTime::now();
    let mut i = 0;
    loop {
        match stream.read(&mut buffer) {
            Ok(n) => {
                if n == 0 {
                    return Err(std::io::Error::new(std::io::ErrorKind::Other, "error"));
                }
                let s = String::from_utf8_lossy(&buffer[..n]).to_string();
                let slice = s.split("\n");
                let count = slice.clone().count();
                for s in slice {
                    i +=1 ;
                    if i == 1 {
                        let v: Vec<&str> = s.split_ascii_whitespace().collect();
                        if v.len() > 2 {
                            request.uri = v.get(1).unwrap().to_string();
                            request.method = v.get(0).unwrap().to_string();
                        }
                    }
                    let mut line = String::from(s);
                    if s.contains("Host: ") {
                        line = String::from("Host: ");
                        line.push_str(proxy_addr.as_str());
                    }
                    if i != count {
                        line.push('\n');
                    }
                    request.request.push_str(line.as_str());
                    target_stream.write(line.as_bytes()).unwrap();
                }
                if n < 1024 {
                    loop {
                        if let Ok(n) = target_stream.read(&mut buffer) {
                            request.response.push_str(String::from_utf8_lossy(&buffer[..n]).to_string().as_str());
                            stream.write(&buffer[..n]).unwrap();
                            if n < 1024 {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    break;
                }
            },
            Err(err) => {
                println!("{}", err);
                return Err(err);
            }
        };
    }
    request.time = std::time::SystemTime::now().duration_since(start_time).unwrap();
    return Ok(request);
}

fn listen(addr: &str, proxy_addr: Arc<String>, requests: Arc<Mutex<Vec<Request>>>) -> bool {
    let pool = ThreadPool::new(4);
    let listener = match TcpListener::bind(addr) {
        Ok(listener) => {
            listener
        },
        Err(err) => {
            println!("{}", err);
            return false;
        }
    };
    std::thread::spawn(move ||{
        for stream in listener.incoming() {
            let requests = Arc::clone(&requests);
            match stream {
                Err(e) => {
                    println!("Err {}", e);
                    return;
                },
                Ok(stream) => {
                    pool.execute(Job::new(stream, requests, Arc::clone(&proxy_addr)));
                }
            }
        }
    });
    return true;
}

fn main() {
    Window::run(Settings {
        antialiasing: true,
        window: iced::window::Settings {
            ..iced::window::Settings::default()
        },
        ..Settings::default()
    }).unwrap();
}

use std::sync::mpsc;

pub struct Job {
    requests: Arc<Mutex<Vec<Request>>>,
    proxy_addr: Arc<String>,
    stream: TcpStream,
}

impl Job {
    fn new(stream: TcpStream, requests: Arc<Mutex<Vec<Request>>>, proxy_addr: Arc<String>) -> Job {
        Job{stream, requests, proxy_addr}
    }

    fn dosomething(&self) {
        loop {
            let request = Request::default();
            let request  = match handle_connection(&self.stream, &self.proxy_addr, request) {
                Ok(req) => {
                    req
                },
                Err(_) => {
                    break;
                }
            };
            let mut requests = self.requests.lock().unwrap();
            if requests.len() > 30 {
                requests.remove(0);
            }
            requests.push(request);
        }
    }
}

struct Worker {
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        std::thread::spawn(move ||loop{
            let job = receiver.lock().unwrap().recv().unwrap();
            let start_time = std::time::SystemTime::now();
            job.dosomething();
            println!("worker {} {:?}", id, std::time::SystemTime::now().duration_since(start_time));
        });
        Worker{}
    }
}

pub struct ThreadPool {
    sender: mpsc::Sender<Job>,
}

impl ThreadPool {
    fn new(n: usize) -> ThreadPool {
        let mut workers = Vec::with_capacity(n);
        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        for id in 0..n {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }
        ThreadPool{ sender }
    }

    pub fn execute(&self, job: Job) {
        self.sender.send(job).unwrap();
    }
}